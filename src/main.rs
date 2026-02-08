use actix_cors::Cors;
use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer};
use request_catcher::{
    capture_request, clear_bucket_requests, create_bucket, delete_bucket, get_bucket_requests,
    list_buckets, AppState,
};
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing subscriber for structured logging
    // Log level can be set with the RUST_LOG environment variable (e.g., RUST_LOG=info,request_catcher=debug)
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let app_state = web::Data::new(AppState {
        buckets: Default::default(),
    });

    // Get host and port from environment variables, with defaults for development
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let address = format!("{}:{}", host, port);

    info!("Server starting on http://{}", address);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(web::PayloadConfig::new(MAX_PAYLOAD_SIZE))
            .service(
                web::scope("/api")
                    .wrap(
                        Cors::default()
                            .allow_any_origin()
                            .allow_any_method()
                            .allow_any_header(),
                    )
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
            .default_service(web::route().to(capture_request))
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
