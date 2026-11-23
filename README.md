# Request Catcher

A simple HTTP Request capture web application written in Rust, largely inspired by [request-baskets](https://github.com/darklynx/request-baskets). Useful for testing HTTP requests and responses such as webhooks, security testing for SSRF, and more.

You can interact with a demo instance of this application at:
- [https://request-catcher.fly.dev/](https://request-catcher.fly.dev/)

## Features
- Complete HTTP request capture: source IP, method, path, raw query string, HTTP version, every header (including repeats), and the exact body bytes
- Custom responses per bucket: set the status, headers, and body, e.g. a `302` redirect to an internal address to test redirect-based SSRF bypasses
- Optional subdomain capture (`<bucket>.catcher.example.com`) for injection points that only accept a hostname
- Copy any captured request as a `curl` command, and download bodies (including binary uploads) as files
- User controlled, password protected buckets for capturing requests privately
- Optional token to restrict who can create buckets
- Ability to view all existing buckets making it easy to return to your space
- Ability to delete a bucket and/or clear all requests from a bucket
- Quick share your bucket link for collaboration

## Installation & Usage

### Pre-built Binaries

Download the latest release for your platform from the [releases page](https://github.com/seanmarpo/request-catcher/releases):

### Docker

Pull and run the pre-built multi-architecture image from GitHub Container Registry:

```bash
# Pull the latest version
docker pull ghcr.io/seanmarpo/request-catcher:latest

# Run the container
docker run -d -p 9090:9090 ghcr.io/seanmarpo/request-catcher:latest

# Or run a specific version
docker run -d -p 9090:9090 ghcr.io/seanmarpo/request-catcher:v0.4.0
```

#### Build from Source (Docker)

The Dockerfile automatically detects your architecture (AMD64 or ARM64) and builds accordingly:

```bash
# Build for your native architecture (auto-detected)
docker build -t request-catcher .

# Run the container
docker run -d -p 9090:9090 request-catcher
```

#### Configuration

Environment variables:
- `HOST` - Bind address (default: `0.0.0.0` in Docker, `127.0.0.1` otherwise)
- `PORT` - Port to listen on (default: `9090`)
- `RUST_LOG` - Log level (default: `info`, options: `error`, `warn`, `info`, `debug`, `trace`)
- `CLIENT_IP_HEADER` - Header to read the client IP from when running behind a reverse proxy (e.g. `Fly-Client-IP` on Fly.io, `X-Real-IP` behind nginx). When unset, or when a request lacks the header, the TCP peer address is used. Only set this if your proxy always overwrites the header, otherwise clients can spoof their IP.
- `BUCKET_DOMAIN` - Enables subdomain capture, e.g. `catcher.example.com`. Requires wildcard DNS; see [Subdomain capture](#subdomain-capture). Unset by default.
- `CREATE_TOKEN` - Require this token (sent as the `X-Create-Token` header, or entered in the UI) to create buckets. Set it to `generate` to have a random token created at startup and printed in the log; it changes on every restart. Capturing requests and viewing buckets never need the token. Unset by default (anyone can create buckets).
- `MAX_BODY_SIZE` - Largest request body to capture, in bytes (default: `10485760`, 10 MB). Larger requests get `413 Payload Too Large` and aren't recorded. Each bucket keeps its latest 1000 requests, so this also bounds memory use.

Example with custom configuration:
```bash
docker run -d \
  -p 8080:8080 \
  -e PORT=8080 \
  -e RUST_LOG=debug \
  ghcr.io/seanmarpo/request-catcher:latest
```

#### Subdomain capture

Some SSRF injection points only accept a hostname, or drop the path, so a request can never reach `/<bucket>`. With `BUCKET_DOMAIN` set, a request to `<bucket>.<BUCKET_DOMAIN>` is captured into `<bucket>` whatever its path, including paths like `/api/...` and `/ui/` that are normally routes.

Setup:
1. Point both the domain and a wildcard at your server, e.g. `A catcher.example.com` and `A *.catcher.example.com`.
2. Run with `BUCKET_DOMAIN=catcher.example.com`. Use the bare domain (`https://catcher.example.com/ui/`) for the UI.
3. For HTTPS you need a wildcard certificate, which requires a DNS-01 challenge (e.g. Caddy or certbot with your DNS provider's plugin, or `fly certs add "*.catcher.example.com"` on Fly.io).

Only bucket names made of lowercase letters, digits, and hyphens work as subdomains (hostnames are case-insensitive and can't contain underscores). Path-based capture keeps working for every bucket.

To try it locally without DNS: `BUCKET_DOMAIN=localhost cargo run`, then `curl http://mybucket.localhost:9090/anything`.

### From Source (Rust)

```bash
# Clone the repository
git clone https://github.com/seanmarpo/request-catcher.git
cd request-catcher

# Run in development mode
cargo run

# Build for production
cargo build --release
./target/release/request_catcher
```

## API

Endpoints that take `X-Bucket-Password` return `401` without a valid password and `404` for unknown buckets.

| Method & path | Auth | Description |
|---|---|---|
| `ANY /<bucket>/...` or `ANY <bucket>.<BUCKET_DOMAIN>/...` | none | Capture a request; replies with the bucket's custom response |
| `POST /api/create/<bucket>` | `X-Create-Token` if `CREATE_TOKEN` is set | Body: `{"password": "..."}` |
| `GET /api/requests/<bucket>?page=1&page_size=50` | `X-Bucket-Password` | Captured requests, newest first (`page_size` max 500) |
| `GET /api/body/<bucket>/<id>` | `X-Bucket-Password` | Exact body bytes of one request, as a download |
| `GET`/`PUT`/`DELETE /api/response/<bucket>` | `X-Bucket-Password` | Read, set, or reset the custom response: `{"status": 302, "headers": [["Location", "http://..."]], "body": ""}` |
| `POST /api/clear/<bucket>` | `X-Bucket-Password` | Delete all captured requests |
| `DELETE /api/delete/<bucket>` | `X-Bucket-Password` | Delete the bucket |
| `GET /api/buckets` | none | List bucket names |
| `GET /api/info` | none | Version and public settings |

Each captured request looks like:

```json
{
  "id": 7,
  "method": "POST",
  "path": "/mybucket/callback",
  "query": "a=1&a=2",
  "http_version": "HTTP/1.1",
  "headers": [["host", "catcher.example.com"], ["x-test", "one"], ["x-test", "two"]],
  "body": "hello",
  "body_size": 5,
  "remote_addr": "203.0.113.7",
  "timestamp": 1795414855517
}
```

`body` is `null` when the body isn't valid UTF-8; fetch the exact bytes from `/api/body/<bucket>/<id>`.

Custom responses always include `Content-Security-Policy: sandbox ...`. They're served from the same origin as the UI, and the sandbox stops them from scripting the UI or registering a service worker. Scripts in them still run.

## Testing

This project includes comprehensive test coverage:

### Integration Tests (Automated)
```bash
# Run all integration tests
cargo test

# Run specific test
cargo test test_create_bucket

# Run with output
cargo test -- --nocapture
```

### UI Tests (Manual/Semi-Automated)
```bash
# Start the server
cargo run

# In another terminal, generate test data
./tests/ui/generate_test_data.py

# Open the provided URL in your browser to verify
```

## AI Disclaimer
Most of this application was "vibe-coded" as a personal exploration project. I do not personally know Rust, but I wanted to see what I could develop with the assistance of AI.
