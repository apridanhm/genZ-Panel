# Build stage
FROM rust:1.76-slim-bookworm as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml ./
COPY crates/core-api/Cargo.toml crates/core-api/
COPY crates/runtime-core/Cargo.toml crates/runtime-core/

# Create dummy main files for build
RUN mkdir -p crates/core-api/src crates/runtime-core/src
RUN echo "fn main() {}" > crates/core-api/src/main.rs
RUN echo "pub fn dummy() {}" > crates/runtime-core/src/lib.rs

# Build dependencies (caching layer)
RUN cargo build --release --package core-api
RUN rm -rf crates/core-api/src

# Copy source code
COPY crates/core-api/src crates/core-api/src
COPY crates/runtime-core/src crates/runtime-core/src

# Build application
RUN cargo build --release --package core-api

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/core-api /app/core-api

# Copy migrations
COPY crates/core-api/migrations /app/migrations

EXPOSE 8000

CMD ["/app/core-api"]