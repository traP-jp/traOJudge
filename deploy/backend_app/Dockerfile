FROM rust:1.95.0-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY server ./server

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --locked --release -p traojudge-backend_app \
    && install -Dm755 /app/target/release/backend_app /out/backend_app

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/backend_app /usr/local/bin/backend_app

ENV TRAOJUDGE_ADDR=0.0.0.0:3000

EXPOSE 3000

USER 65534:65534

CMD ["/usr/local/bin/backend_app"]
