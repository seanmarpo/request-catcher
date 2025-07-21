// main.rs

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio;

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

async fn create_bucket(
    path: web::Path<String>,
    payload: web::Json<CreateBucketPayload>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let bucket_name = path.into_inner();
    let password = payload.into_inner().password;

    if password.is_empty() {
        return HttpResponse::BadRequest().body("Password cannot be empty");
    }

    let mut buckets = app_state.buckets.lock().unwrap();

    if buckets.contains_key(&bucket_name) {
        return HttpResponse::Conflict().body("Bucket already exists");
    }

    let new_bucket = Bucket {
        password,
        requests: Vec::new(),
    };
    buckets.insert(bucket_name, new_bucket);

    HttpResponse::Ok().body("Bucket created")
}

async fn capture_request(
    req: HttpRequest,
    body: web::Bytes,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let path = req.path();
    let bucket_name = match path.trim_start_matches('/').split('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => return HttpResponse::BadRequest().body("Invalid bucket path."),
    };

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
            method,
            headers,
            body,
        };
        bucket.requests.push(request_data);
        HttpResponse::Ok().body("Request captured")
    } else {
        HttpResponse::NotFound().body("Bucket not found")
    }
}

async fn get_bucket_requests(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let bucket_name = req.match_info().get("bucket_name").unwrap_or_default();
    let password = match req.headers().get("X-Bucket-Password") {
        Some(p) => p.to_str().unwrap_or(""),
        None => return HttpResponse::Unauthorized().body("Password required"),
    };

    let buckets = app_state.buckets.lock().unwrap();

    if let Some(bucket) = buckets.get(bucket_name) {
        if bucket.password == password {
            HttpResponse::Ok().json(&bucket.requests)
        } else {
            HttpResponse::Unauthorized().body("Invalid password")
        }
    } else {
        HttpResponse::NotFound().body("Bucket not found")
    }
}

async fn list_buckets(app_state: web::Data<AppState>) -> impl Responder {
    let buckets = app_state.buckets.lock().unwrap();
    let names: Vec<String> = buckets.keys().cloned().collect();
    HttpResponse::Ok().json(names)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        buckets: Mutex::new(HashMap::new()),
    });

    println!("Server running at http://127.0.0.1:9090. Press Ctrl+C to shut down.");

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
    .bind("127.0.0.1:9090")?
    .run();

    let server_handle = server.handle();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        println!("\nCtrl-C received, shutting down gracefully.");
        server_handle.stop(true).await;
    });

    server.await
}
