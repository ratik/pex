# 🏗️ Build stage using Alpine
FROM rust:1.85-alpine AS builder

WORKDIR /usr/src/app

# Install dependencies: OpenSSL, build tools, and pkg-config
RUN apk add --no-cache \
    openssl-dev \
    libssl3 \
    pkgconf \
    build-base \
    clang \
    lld \
    cmake \
    git

# Set OpenSSL environment variables
ENV OPENSSL_DIR=/usr \
    OPENSSL_INCLUDE_DIR=/usr/include \
    OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_STATIC=0 \
    PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_PATH=/usr/lib/pkgconfig \
    PKG_CONFIG_SYSROOT_DIR=/

# Copy dependency files first for caching
COPY Cargo.toml Cargo.lock ./

# Pre-fetch dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true

# Copy actual source code
COPY . .

# Final Rust build
RUN cargo build --release

# 🏗️ Final stage (Minimal Alpine Base)
FROM alpine:latest

# Install OpenSSL runtime for compatibility
RUN apk add --no-cache libssl3

# Copy the built binary
COPY --from=builder /usr/src/app/target/release/portfolio_explorer /usr/local/bin/portfolio_explorer

WORKDIR /

EXPOSE 9100

USER nobody

CMD ["/usr/local/bin/portfolio_explorer"]