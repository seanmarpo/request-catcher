use actix_cors::Cors;
use actix_files::Files;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use subtle::ConstantTimeEq;
use tracing::{error, info, instrument, warn};
use tracing_subscriber::EnvFilter;

// Constants
const PASSWORD_HEADER: &str = "X-Bucket-Password";
const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB

#[derive(Serialize, Deserialize, Clone)]
struct RequestData {
    path: String,
    method: String,
    query_params: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: String,
    timestamp: i64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Bucket {
    password: String,
    requests: Vec<RequestData>,
}

struct AppState {
    buckets: RwLock<HashMap<String, Bucket>>,
}

// Helper function to handle poisoned locks
fn handle_poison<T>(result: Result<T, PoisonError<T>>) -> T {
    match result {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("Lock was poisoned, recovering");
            poisoned.into_inner()
        }
    }
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

// Helper function to authenticate bucket access
fn authenticate_bucket<'a>(
    buckets: &'a RwLockReadGuard<HashMap<String, Bucket>>,
    bucket_name: &str,
    password: &str,
) -> Result<&'a Bucket, HttpResponse> {
    match buckets.get(bucket_name) {
        Some(bucket) => {
            if bucket.password.as_bytes().ct_eq(password.as_bytes()).into() {
                Ok(bucket)
            } else {
                warn!("Invalid password provided for bucket");
                Err(HttpResponse::Unauthorized().body("Invalid password"))
            }
        }
        None => {
            warn!("Request for non-existent bucket");
            Err(HttpResponse::NotFound().body("Bucket not found"))
        }
    }
}

// Helper function to authenticate and get mutable bucket access
fn authenticate_bucket_mut<'a>(
    buckets: &'a mut RwLockWriteGuard<HashMap<String, Bucket>>,
    bucket_name: &str,
    password: &str,
) -> Result<&'a mut Bucket, HttpResponse> {
    match buckets.get_mut(bucket_name) {
        Some(bucket) => {
            if bucket.password.as_bytes().ct_eq(password.as_bytes()).into() {
                Ok(bucket)
            } else {
                error!("Invalid password provided");
                Err(HttpResponse::Unauthorized().body("Invalid password"))
            }
        }
        None => {
            error!("Bucket not found");
            Err(HttpResponse::NotFound().body("Bucket not found"))
        }
    }
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

#[derive(Deserialize)]
struct CreateBucketPayload {
    password: String,
}

#[instrument(skip(app_state, payload), fields(bucket_name = %path.as_str()))]
async fn create_bucket(
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

    let mut buckets = handle_poison(app_state.buckets.write());

    if buckets.contains_key(bucket_name) {
        warn!("Attempted to create a bucket that already exists");
        return HttpResponse::Conflict().body("Bucket already exists");
    }

    let new_bucket = Bucket {
        password,
        requests: Vec::new(),
    };
    buckets.insert(bucket_name.to_string(), new_bucket);

    info!("Successfully created new bucket");
    HttpResponse::Ok().body("Bucket created")
}

#[instrument(skip(req, body, app_state), fields(path = %req.path()))]
async fn capture_request(
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

    let mut buckets = handle_poison(app_state.buckets.write());

    if let Some(bucket) = buckets.get_mut(bucket_name) {
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
        bucket.requests.push(request_data);
        HttpResponse::Ok().body("Request captured")
    } else {
        warn!("Request for non-existent bucket");
        HttpResponse::NotFound().body("Bucket not found")
    }
}

#[instrument(skip(req, app_state), fields(bucket_name = req.match_info().get("bucket_name").unwrap_or("unknown")))]
async fn get_bucket_requests(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match get_password_from_header(&req) {
        Ok(pwd) => pwd,
        Err(response) => return response,
    };

    let buckets = handle_poison(app_state.buckets.read());

    match authenticate_bucket(&buckets, bucket_name, password) {
        Ok(bucket) => HttpResponse::Ok().json(&bucket.requests),
        Err(response) => response,
    }
}

#[instrument(skip(req, app_state), fields(bucket_name = %req.match_info().get("bucket_name").unwrap_or("unknown")))]
async fn delete_bucket(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match get_password_from_header(&req) {
        Ok(pwd) => pwd,
        Err(response) => return response,
    };

    let mut buckets = handle_poison(app_state.buckets.write());

    // First check authentication
    if let Some(bucket) = buckets.get(bucket_name) {
        if bucket.password.as_bytes().ct_eq(password.as_bytes()).into() {
            buckets.remove(bucket_name);
            info!("Successfully deleted bucket");
            HttpResponse::Ok().body("Bucket deleted")
        } else {
            error!("Invalid password provided for deletion");
            HttpResponse::Unauthorized().body("Invalid password")
        }
    } else {
        error!("Bucket not found for deletion");
        HttpResponse::NotFound().body("Bucket not found")
    }
}

#[instrument(skip(req, app_state), fields(bucket_name = %req.match_info().get("bucket_name").unwrap_or("unknown")))]
async fn clear_bucket_requests(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match get_password_from_header(&req) {
        Ok(pwd) => pwd,
        Err(response) => return response,
    };

    let mut buckets = handle_poison(app_state.buckets.write());

    match authenticate_bucket_mut(&mut buckets, bucket_name, password) {
        Ok(bucket) => {
            bucket.requests.clear();
            info!("Successfully cleared requests from bucket");
            HttpResponse::Ok().body("Bucket requests cleared")
        }
        Err(response) => response,
    }
}

async fn list_buckets(app_state: web::Data<AppState>) -> impl Responder {
    let buckets = handle_poison(app_state.buckets.read());
    let names: Vec<&str> = buckets.keys().map(|k| k.as_str()).collect();
    info!(count = names.len(), "Served list of buckets");
    HttpResponse::Ok().json(names)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing subscriber for structured logging
    // Log level can be set with the RUST_LOG environment variable (e.g., RUST_LOG=info,request_catcher=debug)
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let app_state = web::Data::new(AppState {
        buckets: RwLock::new(HashMap::new()),
    });

    // Get host and port from environment variables, with defaults for development
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let address = format!("{}:{}", host, port);

    info!("Server starting on http://{}", address);

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .app_data(web::PayloadConfig::new(MAX_PAYLOAD_SIZE))
            .service(
                web::scope("/api")
                    .route("/buckets", web::get().to(list_buckets))
                    .route(
                        "/clear/{bucket_name}",
                        web::post().to(clear_bucket_requests),
                    )
                    .route("/delete/{bucket_name}", web::delete().to(delete_bucket))
                    .route("/create/{bucket_name}", web::post().to(create_bucket))
                    .route(
                        "/requests/{bucket_name}",
                        web::get().to(get_bucket_requests),
                    ),
            )
            .service(
                web::scope("/ui").service(Files::new("/", "./static").index_file("index.html")),
            )
            .route(
                "/",
                web::get().to(|| async {
                    HttpResponse::Found()
                        .append_header(("Location", "/ui/"))
                        .finish()
                }),
            )
            .route("/{path:.*}", web::route().to(capture_request))
    })
    .bind(&address)?
    .run();

    let server_handle = server.handle();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        info!("Ctrl-C received, shutting down gracefully.");
        server_handle.stop(true).await;
    });

    server.await
}
