use actix_web::http::header::HeaderName;
use actix_web::{web, App, HttpServer};
use request_catcher::{routes, AppState, Config, DEFAULT_MAX_BODY_SIZE};
use std::env;
use std::io::{Error, Read};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Reads an environment variable, treating empty values as unset.
fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// 128-bit random hex token from the OS (Linux/macOS).
fn random_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn config_from_env() -> std::io::Result<Config> {
    let invalid = |msg: &str| Error::new(std::io::ErrorKind::InvalidInput, msg.to_owned());

    let client_ip_header = var("CLIENT_IP_HEADER")
        .map(|h| {
            HeaderName::try_from(h)
                .map_err(|_| invalid("CLIENT_IP_HEADER is not a valid header name"))
        })
        .transpose()?;

    let bucket_domain = var("BUCKET_DOMAIN").map(|d| {
        d.trim()
            .trim_start_matches("*.")
            .trim_matches('.')
            .to_ascii_lowercase()
    });

    let create_token = match var("CREATE_TOKEN") {
        Some(t) if t == "generate" => {
            let token = random_token()?;
            warn!("Bucket creation requires X-Create-Token: {token}");
            Some(token)
        }
        token => token,
    };

    let max_body_size = var("MAX_BODY_SIZE")
        .map(|v| {
            v.trim()
                .parse()
                .map_err(|_| invalid("MAX_BODY_SIZE must be a number of bytes"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_MAX_BODY_SIZE);

    Ok(Config {
        client_ip_header,
        bucket_domain,
        create_token,
        max_body_size,
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Log level comes from RUST_LOG (e.g. RUST_LOG=debug), defaulting to info.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let host = var("HOST").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = var("PORT").unwrap_or_else(|| "9090".to_string());
    let address = format!("{host}:{port}");

    let config = config_from_env()?;
    match &config.client_ip_header {
        Some(h) => info!("Client IP: {h} header, falling back to the TCP peer address"),
        None => info!("Client IP: TCP peer address"),
    }
    if let Some(domain) = &config.bucket_domain {
        info!("Capturing requests to <bucket>.{domain}");
    }
    if config.create_token.is_some() {
        info!("Bucket creation requires a token");
    }
    info!("Max request body: {} bytes", config.max_body_size);

    let state = web::Data::new(AppState::new(config));

    info!("Server starting on http://{address}");

    // actix-web handles SIGINT/SIGTERM itself and shuts down gracefully.
    HttpServer::new(move || App::new().app_data(state.clone()).configure(routes))
        .bind(&address)?
        .run()
        .await
}
