# Dockerfile for fubbik server
# Optimized multi-stage build for Docker deployment

# Build stage
FROM oven/bun:1.3.9 AS builder

WORKDIR /app

# Copy package files for all workspaces
COPY package.json bun.lock* ./
COPY apps/cli/package.json ./apps/cli/
COPY apps/server/package.json ./apps/server/
COPY apps/web/package.json ./apps/web/
COPY packages/api/package.json ./packages/api/
COPY packages/auth/package.json ./packages/auth/
COPY packages/config/package.json ./packages/config/
COPY packages/db/package.json ./packages/db/
COPY packages/env/package.json ./packages/env/

# Install dependencies (secret mount keeps .npmrc out of image layers)
RUN --mount=type=cache,id=fubbik-bun-cache,target=/root/.bun/install/cache \
    --mount=type=secret,id=npmrc,target=/root/.npmrc \
    bun install --frozen-lockfile

# Copy source code
COPY . .

# Build the application
RUN bun run build --filter=server

# Production stage
FROM oven/bun:1.3.9-slim AS runner

WORKDIR /app

ENV NODE_ENV=production
ENV PORT=3000

# Copy package files to preserve workspace resolution
COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/apps/server/package.json ./apps/server/package.json

# Copy node_modules (root has .bun/ with actual packages, server has symlinks)
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/apps/server/node_modules ./apps/server/node_modules

# Copy built application
COPY --from=builder /app/apps/server/dist ./apps/server/dist

EXPOSE 3000

WORKDIR /app/apps/server

CMD ["bun", "run", "dist/index.mjs"]
