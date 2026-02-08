use actix_web::{web, HttpRequest, HttpResponse, Responder};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use subtle::ConstantTimeEq;
use tracing::{error, info, instrument, warn};

// Constants
const PASSWORD_HEADER: &str = "X-Bucket-Password";
const MAX_REQUESTS_PER_BUCKET: usize = 1000;
const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 500;

// Reserved bucket names that cannot be used (conflicts with routes)
const RESERVED_BUCKET_NAMES: &[&str] = &["api", "ui"];

#[derive(Serialize, Deserialize, Clone)]
pub struct RequestData {
    pub path: String,
    pub method: String,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Bucket {
    pub password: String,
    pub requests: Vec<RequestData>,
}

pub struct AppState {
    pub buckets: DashMap<String, Bucket>,
}

// Helper function to extract bucket name from path
fn extract_bucket_name(path: &str) -> Option<&str> {
    path.trim_start_matches('/')
        .split('/')
        .next()
        .filter(|name| !name.is_empty())
}

// Helper function to extract and validate password from request
fn get_password_from_header(req: &HttpRequest) -> Result<&str, HttpResponse> {
    match req.headers().get(PASSWORD_HEADER) {
        Some(p) => Ok(p.to_str().unwrap_or("")),
        None => {
            warn!("Password header missing");
            Err(HttpResponse::Unauthorized().body("Password required"))
        }
    }
}

// Helper function to verify password against bucket
fn verify_bucket_password(bucket: &Bucket, password: &str) -> bool {
    bucket.password.as_bytes().ct_eq(password.as_bytes()).into()
}

// Helper function to parse query parameters
fn parse_query_params(query_string: &str) -> HashMap<String, String> {
    if query_string.is_empty() {
        return HashMap::new();
    }

    query_string
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) if !key.is_empty() => {
                    Some((key.to_string(), value.to_string()))
                }
                (Some(key), None) if !key.is_empty() => Some((key.to_string(), String::new())),
                _ => None,
            }
        })
        .collect()
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

// Helper function to check if bucket name is reserved
fn is_reserved_bucket_name(name: &str) -> bool {
    RESERVED_BUCKET_NAMES.contains(&name)
}

// Helper function to validate bucket name
fn validate_bucket_name(name: &str) -> Result<(), &'static str> {
    // Check if empty
    if name.is_empty() {
        return Err("Bucket name cannot be empty");
    }

    // Check if reserved
    if is_reserved_bucket_name(name) {
        return Err("Bucket name is reserved and cannot be used. Reserved names: api, ui");
    }

    // Check length (reasonable limits)
    if name.len() > 100 {
        return Err("Bucket name is too long (max 100 characters)");
    }

    // Check for valid characters (alphanumeric, hyphens, underscores)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Bucket name can only contain alphanumeric characters, hyphens, and underscores",
        );
    }

    // Check that it doesn't start or end with hyphen/underscore
    if name.starts_with('-') || name.starts_with('_') || name.ends_with('-') || name.ends_with('_')
    {
        return Err("Bucket name cannot start or end with hyphen or underscore");
    }

    Ok(())
}

#[instrument(skip(app_state, payload), fields(bucket_name = %path.as_str()))]
pub async fn create_bucket(
    path: web::Path<String>,
    payload: web::Json<CreateBucketPayload>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let bucket_name = path.as_ref();
    let password = payload.into_inner().password;

    if password.is_empty() {
        warn!("Attempted to create bucket with empty password");
        return HttpResponse::BadRequest().body("Password cannot be empty");
    }

    // Validate bucket name
    if let Err(error_msg) = validate_bucket_name(bucket_name) {
        warn!(
            bucket_name = %bucket_name,
            error = %error_msg,
            "Attempted to create bucket with invalid name"
        );
        return HttpResponse::BadRequest().body(error_msg);
    }

    if app_state.buckets.contains_key(bucket_name) {
        warn!("Attempted to create a bucket that already exists");
        return HttpResponse::Conflict().body("Bucket already exists");
    }

    let new_bucket = Bucket {
        password,
        requests: Vec::new(),
    };
    app_state
        .buckets
        .insert(bucket_name.to_string(), new_bucket);

    info!("Successfully created new bucket");
    HttpResponse::Ok().body("Bucket created")
}

#[instrument(skip(req, body, app_state), fields(path = %req.path()))]
pub async fn capture_request(
    req: HttpRequest,
    body: web::Bytes,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let path = req.path();
    let bucket_name = match extract_bucket_name(path) {
        Some(name) => name,
        None => {
            warn!("Request with invalid bucket path");
            return HttpResponse::BadRequest().body("Invalid bucket path.");
        }
    };
    tracing::Span::current().record("bucket_name", &bucket_name);

    if let Some(mut bucket_ref) = app_state.buckets.get_mut(bucket_name) {
        let method = req.method().as_str();
        let query_params = parse_query_params(req.query_string());
        let headers: HashMap<String, String> = req
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = String::from_utf8_lossy(&body).into_owned();

        let request_data = RequestData {
            path: path.to_string(),
            method: method.to_string(),
            query_params,
            headers,
            body,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        };

        info!(method = %method, "Captured request");

        // Limit the number of requests per bucket
        if bucket_ref.requests.len() >= MAX_REQUESTS_PER_BUCKET {
            bucket_ref.requests.remove(0); // Remove oldest request
        }

        bucket_ref.requests.push(request_data);
        HttpResponse::Ok().body("Request captured")
    } else {
        warn!("Request for non-existent bucket");
        HttpResponse::NotFound().body("Bucket not found")
    }
}

#[instrument(skip(req, app_state, query), fields(bucket_name = req.match_info().get("bucket_name").unwrap_or("unknown")))]
pub async fn get_bucket_requests(
    req: HttpRequest,
    query: web::Query<PaginationParams>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match get_password_from_header(&req) {
        Ok(pwd) => pwd,
        Err(response) => return response,
    };

    match app_state.buckets.get(bucket_name) {
        Some(bucket_ref) => {
            if !verify_bucket_password(&bucket_ref, password) {
                warn!("Invalid password provided for bucket");
                return HttpResponse::Unauthorized().body("Invalid password");
            }

            let total = bucket_ref.requests.len();
            let page = query.page.unwrap_or(1).max(1);
            let page_size = query
                .page_size
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .min(MAX_PAGE_SIZE)
                .max(1);
            let total_pages = (total + page_size - 1) / page_size;

            let start = (page - 1) * page_size;
            let end = (start + page_size).min(total);

            let requests = if start < total {
                bucket_ref.requests[start..end].to_vec()
            } else {
                Vec::new()
            };

            let response = PaginatedResponse {
                requests,
                total,
                page,
                page_size,
                total_pages,
            };

            HttpResponse::Ok().json(response)
        }
        None => {
            warn!("Request for non-existent bucket");
            HttpResponse::NotFound().body("Bucket not found")
        }
    }
}

#[instrument(skip(req, app_state), fields(bucket_name = %req.match_info().get("bucket_name").unwrap_or("unknown")))]
pub async fn delete_bucket(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match get_password_from_header(&req) {
        Ok(pwd) => pwd,
        Err(response) => return response,
    };

    // First check authentication
    if let Some((_, bucket)) = app_state.buckets.remove(bucket_name) {
        if verify_bucket_password(&bucket, password) {
            info!("Successfully deleted bucket");
            HttpResponse::Ok().body("Bucket deleted")
        } else {
            // Re-insert the bucket since password was wrong
            app_state.buckets.insert(bucket_name.to_string(), bucket);
            error!("Invalid password provided for deletion");
            HttpResponse::Unauthorized().body("Invalid password")
        }
    } else {
        error!("Bucket not found for deletion");
        HttpResponse::NotFound().body("Bucket not found")
    }
}

#[instrument(skip(req, app_state), fields(bucket_name = %req.match_info().get("bucket_name").unwrap_or("unknown")))]
pub async fn clear_bucket_requests(
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match get_password_from_header(&req) {
        Ok(pwd) => pwd,
        Err(response) => return response,
    };

    match app_state.buckets.get_mut(bucket_name) {
        Some(mut bucket_ref) => {
            if verify_bucket_password(&bucket_ref, password) {
                bucket_ref.requests.clear();
                info!("Successfully cleared requests from bucket");
                HttpResponse::Ok().body("Bucket requests cleared")
            } else {
                error!("Invalid password provided");
                HttpResponse::Unauthorized().body("Invalid password")
            }
        }
        None => {
            error!("Bucket not found");
            HttpResponse::NotFound().body("Bucket not found")
        }
    }
}

pub async fn list_buckets(app_state: web::Data<AppState>) -> impl Responder {
    let names: Vec<String> = app_state
        .buckets
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    info!(count = names.len(), "Served list of buckets");
    HttpResponse::Ok().json(names)
}
