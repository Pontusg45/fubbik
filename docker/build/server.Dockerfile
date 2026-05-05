# syntax=docker/dockerfile:1
#
# Server image — dependencies cache separately from sources (pnpm fetch + turbo).
# Build from monorepo root: docker build -f docker/build/server.Dockerfile .

FROM node:22-slim AS builder

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

WORKDIR /app

ENV CI=1
ENV PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
ENV NX_DAEMON=false
ENV TURBO_TELEMETRY_DISABLED=1
ENV DOTENV_DISABLE=1
ENV TURBO_CACHE_DIR=/root/.cache/turbo

# --- Manifests only: invalidates rarely ------------------------------------
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/cli/package.json ./apps/cli/
COPY apps/server/package.json ./apps/server/
COPY apps/web/package.json ./apps/web/
COPY apps/vscode/package.json ./apps/vscode/
COPY packages/api/package.json ./packages/api/
COPY packages/auth/package.json ./packages/auth/
COPY packages/config/package.json ./packages/config/
COPY packages/db/package.json ./packages/db/
COPY packages/env/package.json ./packages/env/
COPY packages/mcp/package.json ./packages/mcp/

RUN --mount=type=cache,id=fubbik-pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm fetch

# Install before copying sources so this layer caches on lockfile + package.json only.
RUN --mount=type=cache,id=fubbik-pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --prefer-offline

# --- Sources ---------------------------------------------------------------
COPY turbo.json ./
COPY packages/config/ ./packages/config/
COPY packages/env/ ./packages/env/
COPY packages/db/ ./packages/db/
COPY packages/auth/ ./packages/auth/
COPY packages/api/ ./packages/api/
COPY apps/server/ ./apps/server/

RUN --mount=type=cache,id=fubbik-turbo,target=/root/.cache/turbo \
    pnpm run build --filter=server

RUN --mount=type=cache,id=fubbik-pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm --filter=server deploy --legacy /app/deploy

FROM oven/bun:1.3.10-slim AS runner

WORKDIR /app

ENV NODE_ENV=production
ENV PORT=3000

RUN echo "fubbik:x:1001:1001:fubbik:/app:/bin/sh" >> /etc/passwd && \
    echo "fubbik:x:1001:" >> /etc/group && \
    mkdir -p /app && chown 1001:1001 /app

COPY --from=builder --chown=fubbik:fubbik /app/deploy/node_modules ./node_modules
COPY --from=builder --chown=fubbik:fubbik /app/apps/server/dist ./dist
COPY --from=builder --chown=fubbik:fubbik /app/apps/server/package.json ./package.json
COPY --from=builder --chown=fubbik:fubbik /app/apps/server/entrypoint.sh ./entrypoint.sh

COPY --from=builder --chown=fubbik:fubbik /app/packages/db/src ./packages/db/src
COPY --from=builder --chown=fubbik:fubbik /app/packages/db/package.json ./packages/db/package.json
COPY --from=builder --chown=fubbik:fubbik /app/packages/db/drizzle.config.ts ./packages/db/drizzle.config.ts
COPY --from=builder --chown=fubbik:fubbik /app/packages/env/src ./packages/env/src
COPY --from=builder --chown=fubbik:fubbik /app/packages/env/package.json ./packages/env/package.json

EXPOSE 3000

USER fubbik

ENTRYPOINT ["sh", "entrypoint.sh"]
