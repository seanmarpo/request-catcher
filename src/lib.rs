use actix_web::error::{ErrorBadRequest, ErrorConflict, ErrorNotFound, ErrorUnauthorized};
use actix_web::http::header::{self, HeaderName};
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

const PASSWORD_HEADER: &str = "X-Bucket-Password";
const MAX_REQUESTS_PER_BUCKET: usize = 1000;
const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 500;
const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB

// First path segments that are routes, so they can't be bucket names.
const RESERVED_BUCKET_NAMES: &[&str] = &["api", "ui"];

const INDEX_HTML: &str = include_str!("../static/index.html");
const BUCKET_HTML: &str = include_str!("../static/bucket.html");

#[derive(Serialize, Clone)]
pub struct RequestData {
    /// Per-bucket sequence number, unique for the life of the bucket.
    pub id: u64,
    pub path: String,
    pub method: String,
    /// Raw query string exactly as received (without the leading `?`).
    pub query: String,
    /// Headers as (name, value) pairs; a repeated header appears once per value,
    /// in received order.
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub remote_addr: String,
    /// Milliseconds since the Unix epoch.
    pub timestamp: i64,
}

pub struct Bucket {
    password: String,
    requests: VecDeque<RequestData>,
    next_id: u64,
}

#[derive(Default)]
pub struct AppState {
    pub buckets: Mutex<HashMap<String, Bucket>>,
    /// Header to read the client IP from (e.g. `Fly-Client-IP` behind Fly's
    /// proxy). When unset or absent, the TCP peer address is used.
    pub client_ip_header: Option<HeaderName>,
}

#[derive(Deserialize, Serialize)]
pub struct CreateBucketPayload {
    pub password: String,
}

#[derive(Deserialize)]
pub struct PaginationParams {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Serialize)]
pub struct PaginatedResponse {
    pub requests: Vec<RequestData>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

/// Registers every route. Shared by `main` and the integration tests so both
/// exercise the same routing.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::PayloadConfig::new(MAX_PAYLOAD_SIZE))
        .service(
            web::scope("/api")
                .route("/buckets", web::get().to(list_buckets))
                .route("/version", web::get().to(get_version))
                .route("/create/{bucket_name}", web::post().to(create_bucket))
                .route(
                    "/requests/{bucket_name}",
                    web::get().to(get_bucket_requests),
                )
                .route(
                    "/clear/{bucket_name}",
                    web::post().to(clear_bucket_requests),
                )
                .route("/delete/{bucket_name}", web::delete().to(delete_bucket)),
        )
        .route("/", web::get().to(|| redirect("/ui/")))
        .route("/ui", web::get().to(|| redirect("/ui/")))
        .route("/ui/", web::get().to(|| html(INDEX_HTML)))
        .route("/ui/index.html", web::get().to(|| html(INDEX_HTML)))
        .route("/ui/bucket.html", web::get().to(|| html(BUCKET_HTML)))
        .default_service(web::to(capture_request));
}

async fn redirect(location: &'static str) -> HttpResponse {
    HttpResponse::Found()
        .insert_header((header::LOCATION, location))
        .finish()
}

async fn html(body: &'static str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

fn validate_bucket_name(name: &str) -> Result<(), &'static str> {
    if RESERVED_BUCKET_NAMES.contains(&name) {
        return Err("Bucket name is reserved and cannot be used. Reserved names: api, ui");
    }
    if name.is_empty() || name.len() > 100 {
        return Err("Bucket name must be between 1 and 100 characters");
    }
    // ASCII only: captures match on the raw (percent-encoded) request path,
    // so non-ASCII names could be created but never receive requests.
    if !name
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("Bucket name can only contain ASCII letters, digits, hyphens, and underscores");
    }
    if name.starts_with(['-', '_']) || name.ends_with(['-', '_']) {
        return Err("Bucket name cannot start or end with hyphen or underscore");
    }
    Ok(())
}

/// Looks up a bucket and checks the password header against it.
fn authorize<'a>(
    buckets: &'a mut HashMap<String, Bucket>,
    name: &str,
    req: &HttpRequest,
) -> Result<&'a mut Bucket> {
    let password = req
        .headers()
        .get(PASSWORD_HEADER)
        .ok_or_else(|| ErrorUnauthorized("Password required"))?;
    let bucket = buckets
        .get_mut(name)
        .ok_or_else(|| ErrorNotFound("Bucket not found"))?;
    if bucket.password.as_bytes() != password.as_bytes() {
        warn!(bucket = name, "Invalid password");
        return Err(ErrorUnauthorized("Invalid password"));
    }
    Ok(bucket)
}

fn client_ip(req: &HttpRequest, header: Option<&HeaderName>) -> String {
    header
        .and_then(|h| req.headers().get(h))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_default()
}

pub async fn create_bucket(
    path: web::Path<String>,
    payload: web::Json<CreateBucketPayload>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let name = path.into_inner();
    let password = payload.into_inner().password;

    if password.is_empty() {
        return Err(ErrorBadRequest("Password cannot be empty"));
    }
    validate_bucket_name(&name).map_err(ErrorBadRequest)?;

    match state.buckets.lock().unwrap().entry(name) {
        Entry::Occupied(_) => Err(ErrorConflict("Bucket already exists")),
        Entry::Vacant(entry) => {
            info!(bucket = entry.key(), "Created bucket");
            entry.insert(Bucket {
                password,
                requests: VecDeque::new(),
                next_id: 1,
            });
            Ok(HttpResponse::Ok().body("Bucket created"))
        }
    }
}

pub async fn capture_request(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let path = req.path();
    let name = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default();
    if name.is_empty() {
        return Err(ErrorBadRequest("Invalid bucket path."));
    }

    let remote_addr = client_ip(&req, state.client_ip_header.as_ref());
    let mut buckets = state.buckets.lock().unwrap();
    let Some(bucket) = buckets.get_mut(name) else {
        return Err(ErrorNotFound("Bucket not found"));
    };

    info!(bucket = name, method = %req.method(), path, remote_addr, "Captured request");

    let request = RequestData {
        id: bucket.next_id,
        path: path.to_owned(),
        method: req.method().to_string(),
        query: req.query_string().to_owned(),
        headers: req
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect(),
        body: String::from_utf8_lossy(&body).into_owned(),
        remote_addr,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
    };

    bucket.next_id += 1;
    if bucket.requests.len() >= MAX_REQUESTS_PER_BUCKET {
        bucket.requests.pop_front();
    }
    bucket.requests.push_back(request);
    Ok(HttpResponse::Ok().body("Request captured"))
}

/// Returns a page of captured requests, newest first.
pub async fn get_bucket_requests(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<PaginationParams>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut buckets = state.buckets.lock().unwrap();
    let bucket = authorize(&mut buckets, &path, &req)?;

    let total = bucket.requests.len();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let requests = bucket
        .requests
        .iter()
        .rev()
        .skip((page - 1).saturating_mul(page_size))
        .take(page_size)
        .cloned()
        .collect();

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        requests,
        total,
        page,
        page_size,
        total_pages: total.div_ceil(page_size),
    }))
}

pub async fn clear_bucket_requests(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut buckets = state.buckets.lock().unwrap();
    authorize(&mut buckets, &path, &req)?.requests.clear();
    info!(bucket = %path, "Cleared bucket");
    Ok(HttpResponse::Ok().body("Bucket requests cleared"))
}

pub async fn delete_bucket(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut buckets = state.buckets.lock().unwrap();
    authorize(&mut buckets, &path, &req)?;
    buckets.remove(path.as_str());
    info!(bucket = %path, "Deleted bucket");
    Ok(HttpResponse::Ok().body("Bucket deleted"))
}

pub async fn list_buckets(state: web::Data<AppState>) -> HttpResponse {
    let mut names: Vec<String> = state.buckets.lock().unwrap().keys().cloned().collect();
    names.sort_unstable();
    HttpResponse::Ok().json(names)
}

pub async fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
