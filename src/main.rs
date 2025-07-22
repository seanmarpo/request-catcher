// main.rs

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Mutex;
use tokio;
use tracing::{error, info, instrument, warn};
use tracing_subscriber::EnvFilter;

#[derive(Serialize, Deserialize, Clone)]
struct RequestData {
    path: String,
    method: String,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Bucket {
    password: String,
    requests: Vec<RequestData>,
}

struct AppState {
    buckets: Mutex<HashMap<String, Bucket>>,
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
    let bucket_name = path.into_inner();
    let password = payload.into_inner().password;

    if password.is_empty() {
        warn!("Attempted to create bucket with empty password");
        return HttpResponse::BadRequest().body("Password cannot be empty");
    }

    let mut buckets = app_state.buckets.lock().unwrap();

    if buckets.contains_key(&bucket_name) {
        warn!("Attempted to create a bucket that already exists");
        return HttpResponse::Conflict().body("Bucket already exists");
    }

    let new_bucket = Bucket {
        password,
        requests: Vec::new(),
    };
    buckets.insert(bucket_name.clone(), new_bucket);

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
    let bucket_name = match path.trim_start_matches('/').split('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => {
            warn!("Request with invalid bucket path");
            return HttpResponse::BadRequest().body("Invalid bucket path.");
        }
    };
    tracing::Span::current().record("bucket_name", &bucket_name.as_str());

    let mut buckets = app_state.buckets.lock().unwrap();

    if let Some(bucket) = buckets.get_mut(&bucket_name) {
        let method = req.method().to_string();
        let headers = req
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = String::from_utf8(body.to_vec()).unwrap_or_default();

        let request_data = RequestData {
            path: path.to_string(),
            method: method.clone(),
            headers,
            body,
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
    let password = match req.headers().get("X-Bucket-Password") {
        Some(p) => p.to_str().unwrap_or(""),
        None => {
            warn!("Password header missing");
            return HttpResponse::Unauthorized().body("Password required");
        }
    };

    let buckets = app_state.buckets.lock().unwrap();

    if let Some(bucket) = buckets.get(bucket_name) {
        if bucket.password == password {
            HttpResponse::Ok().json(&bucket.requests)
        } else {
            warn!("Invalid password provided for bucket");
            HttpResponse::Unauthorized().body("Invalid password")
        }
    } else {
        warn!("Request for non-existent bucket");
        HttpResponse::NotFound().body("Bucket not found")
    }
}

#[instrument(skip(app_state))]
#[instrument(skip(req, app_state), fields(bucket_name = %req.match_info().get("bucket_name").unwrap_or("unknown")))]
async fn delete_bucket(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match req.headers().get("X-Bucket-Password") {
        Some(p) => p.to_str().unwrap_or(""),
        None => {
            error!("Password required for deletion but not provided");
            return HttpResponse::Unauthorized().body("Password required");
        }
    };

    let mut buckets = app_state.buckets.lock().unwrap();

    if let Some(bucket) = buckets.get(bucket_name) {
        if bucket.password == password {
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
    let password = match req.headers().get("X-Bucket-Password") {
        Some(p) => p.to_str().unwrap_or(""),
        None => {
            error!("Password required for clearing requests but not provided");
            return HttpResponse::Unauthorized().body("Password required");
        }
    };

    let mut buckets = app_state.buckets.lock().unwrap();

    if let Some(bucket) = buckets.get_mut(bucket_name) {
        if bucket.password == password {
            bucket.requests.clear();
            info!("Successfully cleared requests from bucket");
            HttpResponse::Ok().body("Bucket requests cleared")
        } else {
            error!("Invalid password provided for clearing requests");
            HttpResponse::Unauthorized().body("Invalid password")
        }
    } else {
        error!("Attempted to clear requests from a bucket that does not exist");
        HttpResponse::NotFound().body("Bucket not found")
    }
}

async fn list_buckets(app_state: web::Data<AppState>) -> impl Responder {
    let buckets = app_state.buckets.lock().unwrap();
    let names: Vec<String> = buckets.keys().cloned().collect();
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
        buckets: Mutex::new(HashMap::new()),
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
