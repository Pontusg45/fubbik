# syntax=docker/dockerfile:1
# Build from the monorepo root:
#   docker build -f docker/build/server.Dockerfile .

FROM rust:1.96-bookworm AS builder

WORKDIR /app

ENV SQLX_OFFLINE=true
ENV CARGO_BUILD_JOBS=1

COPY Cargo.toml Cargo.lock ./
COPY .sqlx/ ./.sqlx/
COPY crates/ ./crates/

RUN --mount=type=cache,id=fubbik-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=fubbik-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=fubbik-cargo-target,target=/app/target \
    cargo build --locked --release -p fubbik && \
    cp /app/target/release/fubbik /tmp/fubbik

FROM debian:bookworm-slim AS runner

RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 1001 fubbik && \
    useradd --uid 1001 --gid fubbik --home-dir /app --create-home fubbik

WORKDIR /app

ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000

COPY --from=builder --chown=fubbik:fubbik /tmp/fubbik /usr/local/bin/fubbik

EXPOSE 3000

USER fubbik

ENTRYPOINT ["fubbik"]
CMD ["serve"]
