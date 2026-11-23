use actix_web::error::{
    ErrorBadRequest, ErrorConflict, ErrorNotFound, ErrorPayloadTooLarge, ErrorUnauthorized,
};
use actix_web::guard::{self, GuardContext};
use actix_web::http::header::{self, HeaderName, HeaderValue};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

const PASSWORD_HEADER: &str = "X-Bucket-Password";
const CREATE_TOKEN_HEADER: &str = "X-Create-Token";
const MAX_REQUESTS_PER_BUCKET: usize = 1000;
const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 500;
pub const DEFAULT_MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB

// First path segments that are routes, so they can't be bucket names.
const RESERVED_BUCKET_NAMES: &[&str] = &["api", "ui"];

// Headers a custom response may not set, because the server manages them.
const FORBIDDEN_RESPONSE_HEADERS: &[&str] = &["content-length", "transfer-encoding", "connection"];

// Applied to every capture response and body download. Bucket owners control
// these responses, and they share an origin with the UI: the sandbox gives
// them an opaque origin so they can't script the UI or register a service
// worker, while still letting scripts run (e.g. for headless-browser SSRF).
const SANDBOX_CSP: &str = "sandbox allow-scripts allow-forms allow-popups allow-modals";

const INDEX_HTML: &str = include_str!("../static/index.html");
const BUCKET_HTML: &str = include_str!("../static/bucket.html");

/// Runtime settings, read from environment variables by `main`.
#[derive(Clone)]
pub struct Config {
    /// Header to read the client IP from (e.g. `Fly-Client-IP` behind Fly's
    /// proxy). When unset or absent, the TCP peer address is used.
    pub client_ip_header: Option<HeaderName>,
    /// When set (e.g. `catcher.example.com`), requests to
    /// `<bucket>.catcher.example.com` are captured into `<bucket>`.
    /// Stored lowercase, without a leading `*.` or dot.
    pub bucket_domain: Option<String>,
    /// When set, creating a bucket requires this value in `X-Create-Token`.
    pub create_token: Option<String>,
    /// Largest request body accepted by capture endpoints, in bytes.
    pub max_body_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_ip_header: None,
            bucket_domain: None,
            create_token: None,
            max_body_size: DEFAULT_MAX_BODY_SIZE,
        }
    }
}

impl Config {
    /// Returns the bucket named by a `<bucket>.<bucket_domain>` host, if any.
    fn bucket_from_host(&self, host: &str) -> Option<String> {
        let domain = self.bucket_domain.as_deref()?;
        let host = host.split(':').next()?.to_ascii_lowercase();
        let label = host.strip_suffix(domain)?.strip_suffix('.')?;
        (!label.is_empty() && !label.contains('.')).then(|| label.to_owned())
    }
}

#[derive(Serialize, Clone)]
pub struct RequestData {
    /// Per-bucket sequence number, unique for the life of the bucket.
    pub id: u64,
    pub method: String,
    pub path: String,
    /// Raw query string exactly as received (without the leading `?`).
    pub query: String,
    pub http_version: String,
    /// Headers as (name, value) pairs; a repeated header appears once per value,
    /// in received order.
    pub headers: Vec<(String, String)>,
    /// The body as text, or `null` if it isn't valid UTF-8. The exact bytes
    /// are always available from `/api/body/{bucket}/{id}`.
    #[serde(serialize_with = "utf8_or_null")]
    pub body: web::Bytes,
    pub body_size: usize,
    pub remote_addr: String,
    /// Milliseconds since the Unix epoch.
    pub timestamp: i64,
}

fn utf8_or_null<S: Serializer>(body: &web::Bytes, s: S) -> Result<S::Ok, S::Error> {
    match std::str::from_utf8(body) {
        Ok(text) => s.serialize_some(text),
        Err(_) => s.serialize_none(),
    }
}

/// What a bucket sends back to whoever hits its capture URL.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct CustomResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: String,
}

impl Default for CustomResponse {
    fn default() -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: "Request captured".to_owned(),
        }
    }
}

impl CustomResponse {
    fn validate(&self) -> Result<(), String> {
        // 1xx would break the connection; anything outside 200-599 isn't real
        if !(200..=599).contains(&self.status) {
            return Err(format!(
                "Status must be between 200 and 599, got {}",
                self.status
            ));
        }
        for (name, value) in &self.headers {
            let parsed = HeaderName::try_from(name.as_str())
                .map_err(|_| format!("Invalid header name: {name:?}"))?;
            if FORBIDDEN_RESPONSE_HEADERS.contains(&parsed.as_str()) {
                return Err(format!("Header {name:?} is set by the server"));
            }
            HeaderValue::try_from(value.as_str())
                .map_err(|_| format!("Invalid value for header {name:?}"))?;
        }
        Ok(())
    }

    fn to_http(&self) -> HttpResponse {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK);
        let mut resp = HttpResponse::build(status);
        for (name, value) in &self.headers {
            resp.append_header((name.as_str(), value.as_str()));
        }
        resp.insert_header((header::CONTENT_SECURITY_POLICY, SANDBOX_CSP))
            .body(self.body.clone())
    }
}

pub struct Bucket {
    password: String,
    requests: VecDeque<RequestData>,
    next_id: u64,
    response: CustomResponse,
}

#[derive(Default)]
pub struct AppState {
    pub buckets: Mutex<HashMap<String, Bucket>>,
    pub config: Config,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
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
/// exercise the same routing. Expects `web::Data<AppState>` as app data.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        // On a bucket subdomain every path is captured, including /api and /ui.
        web::scope("")
            .guard(guard::fn_guard(|ctx| !is_bucket_host(ctx)))
            .service(
                web::scope("/api")
                    .route("/info", web::get().to(get_info))
                    .route("/version", web::get().to(get_version))
                    .route("/buckets", web::get().to(list_buckets))
                    .route("/create/{bucket}", web::post().to(create_bucket))
                    .route("/requests/{bucket}", web::get().to(get_bucket_requests))
                    .route("/body/{bucket}/{id}", web::get().to(download_body))
                    .route("/clear/{bucket}", web::post().to(clear_bucket_requests))
                    .route("/delete/{bucket}", web::delete().to(delete_bucket))
                    .service(
                        web::resource("/response/{bucket}")
                            .route(web::get().to(get_response))
                            .route(web::put().to(set_response))
                            .route(web::delete().to(reset_response)),
                    ),
            )
            .route("/", web::get().to(|| redirect("/ui/")))
            .route("/ui", web::get().to(|| redirect("/ui/")))
            .route("/ui/", web::get().to(|| html(INDEX_HTML)))
            .route("/ui/index.html", web::get().to(|| html(INDEX_HTML)))
            .route("/ui/bucket.html", web::get().to(|| html(BUCKET_HTML))),
    )
    .default_service(web::to(capture_request));
}

fn request_host(head: &actix_web::dev::RequestHead) -> Option<&str> {
    head.headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .or_else(|| head.uri.host()) // HTTP/2 sends :authority instead of Host
}

fn is_bucket_host(ctx: &GuardContext) -> bool {
    let Some(state) = ctx.app_data::<web::Data<AppState>>() else {
        return false;
    };
    request_host(ctx.head()).is_some_and(|h| state.config.bucket_from_host(h).is_some())
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
    if name.is_empty() || name.len() > 63 {
        return Err("Bucket name must be between 1 and 63 characters");
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
    req: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<CreateBucketPayload>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    if let Some(token) = &state.config.create_token {
        let sent = req.headers().get(CREATE_TOKEN_HEADER);
        if sent.map(|v| v.as_bytes()) != Some(token.as_bytes()) {
            warn!("Bucket creation with missing or invalid token");
            return Err(ErrorUnauthorized("A valid X-Create-Token is required"));
        }
    }

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
                response: CustomResponse::default(),
            });
            Ok(HttpResponse::Ok().body("Bucket created"))
        }
    }
}

pub async fn capture_request(
    req: HttpRequest,
    payload: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let path = req.path();
    let host_bucket = request_host(req.head()).and_then(|h| state.config.bucket_from_host(h));
    let name = match &host_bucket {
        Some(name) => name.as_str(),
        None => path
            .trim_start_matches('/')
            .split('/')
            .next()
            .unwrap_or_default(),
    };
    if name.is_empty() {
        return Err(ErrorBadRequest("Invalid bucket path."));
    }

    let body = match payload.to_bytes_limited(state.config.max_body_size).await {
        Ok(body) => body?,
        Err(_) => return Err(ErrorPayloadTooLarge("Request body too large")),
    };
    let remote_addr = client_ip(&req, state.config.client_ip_header.as_ref());

    let mut buckets = state.buckets.lock().unwrap();
    let Some(bucket) = buckets.get_mut(name) else {
        return Err(ErrorNotFound("Bucket not found"));
    };

    info!(bucket = name, method = %req.method(), path, remote_addr, "Captured request");

    let request = RequestData {
        id: bucket.next_id,
        method: req.method().to_string(),
        path: path.to_owned(),
        query: req.query_string().to_owned(),
        http_version: format!("{:?}", req.version()),
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
        body_size: body.len(),
        body,
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
    Ok(bucket.response.to_http())
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

/// Returns a captured request's exact body bytes as a file download.
pub async fn download_body(
    req: HttpRequest,
    path: web::Path<(String, u64)>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let (name, id) = path.into_inner();
    let mut buckets = state.buckets.lock().unwrap();
    let bucket = authorize(&mut buckets, &name, &req)?;
    // Requests are stored in ascending id order
    let index = bucket
        .requests
        .binary_search_by_key(&id, |r| r.id)
        .map_err(|_| ErrorNotFound("Request not found"))?;

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}-{id}.bin\""),
        ))
        .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .insert_header((header::CONTENT_SECURITY_POLICY, SANDBOX_CSP))
        .body(bucket.requests[index].body.clone()))
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

pub async fn get_response(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut buckets = state.buckets.lock().unwrap();
    let bucket = authorize(&mut buckets, &path, &req)?;
    Ok(HttpResponse::Ok().json(&bucket.response))
}

pub async fn set_response(
    req: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<CustomResponse>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let response = payload.into_inner();
    response.validate().map_err(ErrorBadRequest)?;
    let mut buckets = state.buckets.lock().unwrap();
    authorize(&mut buckets, &path, &req)?.response = response;
    info!(bucket = %path, "Updated custom response");
    Ok(HttpResponse::Ok().body("Response updated"))
}

pub async fn reset_response(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut buckets = state.buckets.lock().unwrap();
    authorize(&mut buckets, &path, &req)?.response = CustomResponse::default();
    Ok(HttpResponse::Ok().body("Response reset"))
}

pub async fn list_buckets(state: web::Data<AppState>) -> HttpResponse {
    let mut names: Vec<String> = state.buckets.lock().unwrap().keys().cloned().collect();
    names.sort_unstable();
    HttpResponse::Ok().json(names)
}

#[derive(Serialize)]
struct Info<'a> {
    version: &'static str,
    create_token_required: bool,
    bucket_domain: Option<&'a str>,
    max_body_size: usize,
}

/// Public settings the UI needs.
pub async fn get_info(state: web::Data<AppState>) -> HttpResponse {
    let config = &state.config;
    HttpResponse::Ok().json(Info {
        version: env!("CARGO_PKG_VERSION"),
        create_token_required: config.create_token.is_some(),
        bucket_domain: config.bucket_domain.as_deref(),
        max_body_size: config.max_body_size,
    })
}

pub async fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
