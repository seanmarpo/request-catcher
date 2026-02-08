# Build stage with cargo-chef for dependency caching
FROM rust:latest AS chef

# Install cargo-chef
RUN cargo install cargo-chef

WORKDIR /app

# Prepare the build plan
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Build dependencies
FROM chef AS builder

# Detect target architecture and set appropriate Rust target
ARG TARGETARCH
RUN case "$TARGETARCH" in \
    "amd64")  echo "x86_64-unknown-linux-musl" > /tmp/rust_target ;; \
    "arm64")  echo "aarch64-unknown-linux-musl" > /tmp/rust_target ;; \
    *)        echo "x86_64-unknown-linux-musl" > /tmp/rust_target ;; \
    esac && \
    export RUST_TARGET=$(cat /tmp/rust_target) && \
    echo "Building for target: $RUST_TARGET" && \
    rustup target add $RUST_TARGET

# Install build dependencies for static musl builds
RUN apt-get update && \
    apt-get install -y musl-tools musl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build dependencies (this is cached)
COPY --from=planner /app/recipe.json recipe.json
RUN export RUST_TARGET=$(cat /tmp/rust_target) && \
    cargo chef cook --release --target $RUST_TARGET --recipe-path recipe.json

# Copy source and build application
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static

# Build static binary with musl for Alpine compatibility
# Note: Binary is already stripped via strip = true in Cargo.toml
RUN export RUST_TARGET=$(cat /tmp/rust_target) && \
    cargo build --release --target $RUST_TARGET && \
    cp target/$RUST_TARGET/release/request_catcher /tmp/request_catcher

# Runtime stage - minimal Alpine image
FROM alpine:latest

# Install CA certificates for HTTPS
RUN apk add --no-cache ca-certificates && \
    addgroup -g 1000 appuser && \
    adduser -D -u 1000 -G appuser appuser

WORKDIR /app

# Copy binary and static files
COPY --from=builder /tmp/request_catcher /usr/local/bin/request_catcher
COPY --from=builder /app/static /app/static

# Set ownership
RUN chown -R appuser:appuser /app

USER appuser

# Expose the default port
EXPOSE 9090

# Set environment variables
ENV HOST=0.0.0.0
ENV PORT=9090
ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/request_catcher"]
