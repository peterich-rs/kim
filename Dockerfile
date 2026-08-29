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

RUN cargo build --release -p fake-gateway --features consul \
    && cargo build --release -p fake-chat --features redis,postgres,consul \
    && cargo build --release -p fake-royal --features postgres,redis,consul \
    && cargo build --release -p fake-router --features consul \
    && strip target/release/fake-gateway target/release/fake-chat \
         target/release/fake-royal target/release/fake-router

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin kim

COPY --from=builder /src/target/release/fake-gateway /usr/local/bin/fake-gateway
COPY --from=builder /src/target/release/fake-chat /usr/local/bin/fake-chat
COPY --from=builder /src/target/release/fake-royal /usr/local/bin/fake-royal
COPY --from=builder /src/target/release/fake-router /usr/local/bin/fake-router
COPY deploy/chat.toml deploy/gateway.toml deploy/royal.toml deploy/router.toml /etc/kim/

USER kim
WORKDIR /
CMD ["fake-chat", "/etc/kim/chat.toml"]
