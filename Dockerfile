# linux/amd64 image with gateway / chat / royal / router.
# Build: docker build -t ghcr.io/<owner>/kim:local .
FROM rust:1.95.0-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ make pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY examples ./examples

ENV CARGO_TERM_COLOR=always \
    CARGO_INCREMENTAL=0

RUN cargo build --release -p gateway --features consul \
    && cargo build --release -p chat --features redis,postgres,consul \
    && cargo build --release -p royal --features postgres,redis,consul \
    && cargo build --release -p router --features consul \
    && strip target/release/gateway target/release/chat \
         target/release/royal target/release/router

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin kim

COPY --from=builder /src/target/release/gateway /usr/local/bin/gateway
COPY --from=builder /src/target/release/chat /usr/local/bin/chat
COPY --from=builder /src/target/release/royal /usr/local/bin/royal
COPY --from=builder /src/target/release/router /usr/local/bin/router
COPY deploy/chat.toml deploy/gateway.toml deploy/royal.toml deploy/router.toml /etc/kim/

USER kim
WORKDIR /
CMD ["chat", "/etc/kim/chat.toml"]
