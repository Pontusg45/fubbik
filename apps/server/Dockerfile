# Dockerfile for fubbik server
# Optimized multi-stage build with aggressive caching

# Build stage
FROM node:22-slim AS builder

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

WORKDIR /app

# 1. Copy ALL package files first (cache layer for deps)
#    pnpm --frozen-lockfile validates every workspace package.json against the lockfile
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

RUN PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 pnpm install --frozen-lockfile

# 2. Copy only source needed for the server build (not web/cli/vscode/docs)
COPY packages/config/ ./packages/config/
COPY packages/env/ ./packages/env/
COPY packages/db/ ./packages/db/
COPY packages/auth/ ./packages/auth/
COPY packages/api/ ./packages/api/
COPY apps/server/ ./apps/server/
COPY turbo.json ./

# 3. Build only the server
RUN pnpm run build --filter=server

# 4. Prune dev dependencies and unused workspace deps
RUN pnpm prune --prod --no-optional && \
    rm -rf apps/cli apps/web apps/vscode

# Production stage
FROM oven/bun:1.3.10-slim AS runner

WORKDIR /app

ENV NODE_ENV=production
ENV PORT=3000

# Copy pruned node_modules (single layer instead of 4)
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./package.json

# Copy built application
COPY --from=builder /app/apps/server/dist ./apps/server/dist
COPY --from=builder /app/apps/server/package.json ./apps/server/package.json
COPY --from=builder /app/apps/server/entrypoint.sh ./apps/server/entrypoint.sh

# Copy db package for migrations and seeding (bun runs TS natively)
COPY --from=builder /app/packages/db/src ./packages/db/src
COPY --from=builder /app/packages/db/package.json ./packages/db/package.json
COPY --from=builder /app/packages/db/drizzle.config.ts ./packages/db/drizzle.config.ts
COPY --from=builder /app/packages/env/src ./packages/env/src
COPY --from=builder /app/packages/env/package.json ./packages/env/package.json

EXPOSE 3000

WORKDIR /app/apps/server

ENTRYPOINT ["sh", "entrypoint.sh"]
