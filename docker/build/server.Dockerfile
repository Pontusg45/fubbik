# syntax=docker/dockerfile:1
# Build from the monorepo root:
#   docker build -f docker/build/server.Dockerfile .

FROM node:22-slim AS web-builder

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate
WORKDIR /app
ENV CI=1 PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 DOTENV_DISABLE=1

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json ./apps/web/
COPY apps/vscode/package.json ./apps/vscode/
COPY packages/client/package.json ./packages/client/
COPY packages/config/package.json ./packages/config/
COPY packages/db/package.json ./packages/db/
COPY packages/env/package.json ./packages/env/

RUN --mount=type=cache,id=fubbik-pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm fetch && pnpm install --frozen-lockfile --prefer-offline

COPY packages/client/ ./packages/client/
COPY packages/config/ ./packages/config/
COPY packages/env/ ./packages/env/
COPY apps/web/ ./apps/web/

ARG VITE_FUBBIK_IMPLICIT_DEV_SESSION=true
ENV VITE_FUBBIK_IMPLICIT_DEV_SESSION=${VITE_FUBBIK_IMPLICIT_DEV_SESSION}
RUN pnpm --filter web run build

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

LABEL org.opencontainers.image.title="Fubbik" \
      org.opencontainers.image.description="Rust API, CLI, MCP server, and static web application" \
      org.opencontainers.image.licenses="MIT"

RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 1001 fubbik && \
    useradd --uid 1001 --gid fubbik --home-dir /app --create-home fubbik

WORKDIR /app

ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000
ENV FUBBIK_WEB_ASSETS_DIR=/app/web

COPY --from=builder --chown=fubbik:fubbik /tmp/fubbik /usr/local/bin/fubbik
COPY --from=web-builder --chown=fubbik:fubbik /app/apps/web/dist /app/web

# A fork's committed `fubbik.toml` lands in the WORKDIR, where `fubbik serve`
# reads `./fubbik.toml`. COPY cannot be conditional, so the always-present
# example is the anchor and the `fubbik.toml*` glob adds the real file only
# when it exists. The example itself is never read. To use a file outside the
# image instead, mount it and set `FUBBIK_CONFIG` to its path.
COPY --chown=fubbik:fubbik fubbik.example.toml fubbik.toml* ./

EXPOSE 3000

USER fubbik

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:3000/api/health >/dev/null || exit 1

ENTRYPOINT ["fubbik"]
CMD ["serve"]
