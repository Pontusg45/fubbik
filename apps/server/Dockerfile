# Dockerfile for fubbik server
# Optimized multi-stage build for Docker deployment

# Build stage
FROM node:22-slim AS builder

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

WORKDIR /app

# Copy package files for all workspaces
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/cli/package.json ./apps/cli/
COPY apps/server/package.json ./apps/server/
COPY apps/web/package.json ./apps/web/
COPY packages/api/package.json ./packages/api/
COPY packages/auth/package.json ./packages/auth/
COPY packages/config/package.json ./packages/config/
COPY packages/db/package.json ./packages/db/
COPY packages/env/package.json ./packages/env/

RUN PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 pnpm install --frozen-lockfile

# Copy source code
COPY . .

# Build the application
RUN pnpm run build --filter=server

# Production stage
FROM oven/bun:1.3.10-slim AS runner

WORKDIR /app

ENV NODE_ENV=production
ENV PORT=3000

# Copy package files to preserve workspace resolution
COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/apps/server/package.json ./apps/server/package.json
COPY --from=builder /app/packages/db/package.json ./packages/db/package.json
COPY --from=builder /app/packages/env/package.json ./packages/env/package.json

# Copy node_modules
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/apps/server/node_modules ./apps/server/node_modules
COPY --from=builder /app/packages/db/node_modules ./packages/db/node_modules
COPY --from=builder /app/packages/env/node_modules ./packages/env/node_modules

# Copy built application
COPY --from=builder /app/apps/server/dist ./apps/server/dist

# Copy db package for migrations and seeding (bun runs TS natively)
COPY --from=builder /app/packages/db/src ./packages/db/src
COPY --from=builder /app/packages/db/drizzle.config.ts ./packages/db/drizzle.config.ts
COPY --from=builder /app/packages/env/src ./packages/env/src

# Copy entrypoint script
COPY --from=builder /app/apps/server/entrypoint.sh ./apps/server/entrypoint.sh

EXPOSE 3000

WORKDIR /app/apps/server

ENTRYPOINT ["sh", "entrypoint.sh"]
