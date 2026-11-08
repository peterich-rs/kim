# syntax=docker/dockerfile:1
# Build the isolated context, rather than the repository root:
# python3 tools/build/prepare-server-context.py /tmp/kim-server-build
# docker build -t ghcr.io/<owner>/kim:local /tmp/kim-server-build
FROM rust:1.95.0-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ make pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/shared ./crates/shared
COPY crates/server ./crates/server
COPY services ./services
COPY examples/kimbench ./examples/kimbench
COPY examples/pkt-client ./examples/pkt-client
COPY tools/kim-server-migrate ./tools/kim-server-migrate
ENV CARGO_TERM_COLOR=always CARGO_INCREMENTAL=0

# default-members selects the five services and migrator, not test helpers.
RUN --mount=type=cache,id=kim-server-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=kim-server-target,target=/src/target,sharing=locked \
    cargo build --release --locked --all-features \
    && mkdir /out \
    && for bin in gateway tgateway chat royal router kim-server-migrate; do \
         install -m 755 "target/release/$bin" "/out/$bin"; \
         strip "/out/$bin"; \
       done

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin kim

COPY --from=builder /out/ /usr/local/bin/
COPY deploy/*.toml /etc/kim/
USER kim
WORKDIR /
CMD ["chat", "/etc/kim/chat.toml"]
