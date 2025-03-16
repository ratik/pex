FROM rust:1.85 AS builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y musl-tools

RUN rustup target add x86_64-unknown-linux-musl

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl || true
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM gcr.io/distroless/static-debian12

COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/portfolio_explorer /usr/local/bin/portfolio_explorer

WORKDIR /

EXPOSE 9100

USER nonroot:nonroot

CMD ["/usr/local/bin/portfolio_explorer"]