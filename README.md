# Request Catcher

A simple HTTP Request capture web application written in Rust, largely inspired by [request-baskets](https://github.com/darklynx/request-baskets). Useful for testing HTTP requests and responses such as webhooks, security testing for SSRF, and more.

You can interact with a demo instance of this application at:
- [https://request-catcher.fly.dev/](https://request-catcher.fly.dev/)

## Features
- Complete HTTP request capture including methods, headers, parameters, and body
- User controlled, password protected buckets for capturing requests privately
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

Example with custom configuration:
```bash
docker run -d \
  -p 8080:8080 \
  -e PORT=8080 \
  -e RUST_LOG=debug \
  ghcr.io/seanmarpo/request-catcher:latest
```

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
cd tests/ui
./generate_test_data.sh

# Open the provided URL in your browser to verify
```

## AI Disclaimer
Most of this application was "vibe-coded" as a personal exploration project. I do not personally know Rust, but I wanted to see what I could develop with the assistance of AI.
