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

# Copy source and build application (static/ is embedded into the binary)
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static

# Build a fully static musl binary
# Note: Binary is already stripped via strip = true in Cargo.toml
RUN export RUST_TARGET=$(cat /tmp/rust_target) && \
    cargo build --release --target $RUST_TARGET && \
    cp target/$RUST_TARGET/release/request_catcher /tmp/request_catcher

# Runtime stage - the static binary needs nothing else (UI is embedded,
# and the app makes no outbound connections)
FROM scratch

COPY --from=builder /tmp/request_catcher /request_catcher

# Run as an unprivileged user
USER 1000:1000

# Expose the default port
EXPOSE 9090

# Set environment variables
ENV HOST=0.0.0.0
ENV PORT=9090
ENV RUST_LOG=info

ENTRYPOINT ["/request_catcher"]
