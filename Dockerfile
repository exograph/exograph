# Multi-stage Dockerfile for building exograph exo-server

ARG RUST_VERSION=1.92
FROM rust:${RUST_VERSION}-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    git \
    curl \
    clang \
    libclang-dev \
    libsqlite3-dev \
    libffi-dev \
    build-essential \
    ca-certificates \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy source files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY libs ./libs
COPY playground ./playground

# Install npm dependencies
RUN if [ -f crates/introspection-util/package.json ]; then cd crates/introspection-util && npm ci; fi
RUN if [ -f playground/lib/package.json ]; then cd playground/lib && npm ci; fi

# Ensure npm is in PATH for build scripts
ENV PATH="/usr/bin:${PATH}"

# Build exo-server
RUN cargo build --release --bin exo-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary
COPY --from=builder /build/target/release/exo-server /usr/local/bin/exo-server

# Set the entrypoint
ENTRYPOINT ["exo-server"]
