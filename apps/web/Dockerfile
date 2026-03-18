# Dockerfile for fubbik TanStack Start app
# Optimized multi-stage build with aggressive caching

FROM node:22-slim AS builder

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

WORKDIR /app

# 1. Copy ALL package files first (cache layer for deps)
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

# 2. Copy only source needed for the web build
COPY packages/config/ ./packages/config/
COPY packages/env/ ./packages/env/
COPY packages/db/ ./packages/db/
COPY packages/auth/ ./packages/auth/
COPY packages/api/ ./packages/api/
COPY apps/web/ ./apps/web/
COPY turbo.json ./

# Vite inlines VITE_* at build time
ARG VITE_SERVER_URL
ENV VITE_SERVER_URL=${VITE_SERVER_URL}

# 3. Build only the web app
RUN pnpm run build --filter=web

FROM oven/bun:1.3.10-slim AS runner

WORKDIR /app

ENV NODE_ENV=production
ENV PORT=3000

# Copy node_modules (root + workspace-level for pnpm symlink resolution)
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/apps/web/node_modules ./apps/web/node_modules
COPY --from=builder /app/apps/web/package.json ./apps/web/package.json

# Copy built output
COPY --from=builder /app/apps/web/dist ./apps/web/dist

EXPOSE 3000

WORKDIR /app/apps/web

CMD ["bun", "run", "dist/server/entry-server.js"]
