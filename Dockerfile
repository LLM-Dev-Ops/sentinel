# Multi-stage Dockerfile for LLM-Sentinel
# Optimized for production deployment with minimal image size

# Stage 1: Build dependencies (cached layer)
FROM rust:1.85-slim-bookworm AS chef

# Install build dependencies needed for cargo-chef cook and rdkafka
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    cmake \
    build-essential \
    libsasl2-dev \
    libcurl4-openssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Prepare recipe
FROM chef AS planner
COPY . .
# Pin time crate to version compatible with Rust 1.85
RUN cargo update -p time --precise 0.3.36 || true
RUN cargo update -p time-core --precise 0.1.2 || true
RUN cargo update -p time-macros --precise 0.2.18 || true
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build dependencies (cached)
FROM chef AS builder-deps
COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/Cargo.lock Cargo.lock
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 4: Build application
FROM rust:1.85-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    cmake \
    build-essential \
    libsasl2-dev \
    libcurl4-openssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy cached dependencies
COPY --from=builder-deps /app/target target
COPY --from=builder-deps /usr/local/cargo /usr/local/cargo

# Copy source code and lockfile from planner (with pinned deps)
COPY Cargo.toml ./
COPY --from=planner /app/Cargo.lock ./
COPY crates ./crates
COPY sentinel ./sentinel

# Build release binary
RUN cargo build --release --bin sentinel

# Stage 5: Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies (must match libraries linked during build)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    libcurl4 \
    libzstd1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash sentinel

# Create directories
RUN mkdir -p /etc/sentinel /var/lib/sentinel/baselines /var/log/sentinel && \
    chown -R sentinel:sentinel /etc/sentinel /var/lib/sentinel /var/log/sentinel

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/sentinel /usr/local/bin/sentinel

# Copy default configuration
COPY config/sentinel.yaml /etc/sentinel/sentinel.yaml

# Switch to non-root user
USER sentinel

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8080/health/live || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV SENTINEL_CONFIG=/etc/sentinel/sentinel.yaml
ENV SENTINEL_API_ONLY=true

# Run the application
ENTRYPOINT ["/usr/local/bin/sentinel"]
CMD ["--config", "/etc/sentinel/sentinel.yaml"]
