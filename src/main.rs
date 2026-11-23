use actix_web::http::header::HeaderName;
use actix_web::{web, App, HttpServer};
use request_catcher::{routes, AppState};
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Log level comes from RUST_LOG (e.g. RUST_LOG=debug), defaulting to info.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let address = format!("{host}:{port}");

    let client_ip_header = env::var("CLIENT_IP_HEADER")
        .ok()
        .filter(|h| !h.is_empty())
        .map(|h| HeaderName::try_from(h).expect("CLIENT_IP_HEADER is not a valid header name"));
    match &client_ip_header {
        Some(h) => info!("Reading client IP from the {h} header"),
        None => info!("Using the TCP peer address as the client IP"),
    }

    let state = web::Data::new(AppState {
        client_ip_header,
        ..Default::default()
    });

    info!("Server starting on http://{address}");

    // actix-web handles SIGINT/SIGTERM itself and shuts down gracefully.
    HttpServer::new(move || App::new().app_data(state.clone()).configure(routes))
        .bind(&address)?
        .run()
        .await
}
