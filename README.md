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

## Usage
- `cargo run` to start the application

### Docker Usage
- `docker build -t request-catcher .`
- `docker run -p 9090:9090 request-catcher`

## Testing

This project includes comprehensive test coverage:

### Integration Tests (Automated)
33 Rust integration tests covering backend functionality:
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
