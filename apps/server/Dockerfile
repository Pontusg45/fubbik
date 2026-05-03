# Dockerfile for fubbik server
# Uses pnpm deploy for minimal, flat node_modules (no symlinks)

FROM node:22-slim AS builder

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

WORKDIR /app

# 1. Copy package files (cache layer for deps)
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

RUN --mount=type=cache,id=pnpm,target=/root/.local/share/pnpm/store \
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 pnpm install --frozen-lockfile

# 2. Copy only source needed for the server build
COPY packages/config/ ./packages/config/
COPY packages/env/ ./packages/env/
COPY packages/db/ ./packages/db/
COPY packages/auth/ ./packages/auth/
COPY packages/api/ ./packages/api/
COPY apps/server/ ./apps/server/
COPY turbo.json ./

# 3. Build the server (tsdown bundles workspace deps)
RUN pnpm run build --filter=server

# 4. Deploy: creates flat node_modules with only server's deps (no symlinks)
RUN pnpm --filter=server deploy --legacy /app/deploy

# Production stage
FROM oven/bun:1.3.10-slim AS runner

WORKDIR /app

ENV NODE_ENV=production
ENV PORT=3000

# Create non-root user (portable: works on any base image)
RUN echo "fubbik:x:1001:1001:fubbik:/app:/bin/sh" >> /etc/passwd && \
    echo "fubbik:x:1001:" >> /etc/group && \
    mkdir -p /app && chown 1001:1001 /app

# Copy the deployed flat node_modules (no pnpm symlinks)
COPY --from=builder --chown=fubbik:fubbik /app/deploy/node_modules ./node_modules

# Copy built application
COPY --from=builder --chown=fubbik:fubbik /app/apps/server/dist ./dist
COPY --from=builder --chown=fubbik:fubbik /app/apps/server/package.json ./package.json
COPY --from=builder --chown=fubbik:fubbik /app/apps/server/entrypoint.sh ./entrypoint.sh

# Copy db package for migrations and seeding
COPY --from=builder --chown=fubbik:fubbik /app/packages/db/src ./packages/db/src
COPY --from=builder --chown=fubbik:fubbik /app/packages/db/package.json ./packages/db/package.json
COPY --from=builder --chown=fubbik:fubbik /app/packages/db/drizzle.config.ts ./packages/db/drizzle.config.ts
COPY --from=builder --chown=fubbik:fubbik /app/packages/env/src ./packages/env/src
COPY --from=builder --chown=fubbik:fubbik /app/packages/env/package.json ./packages/env/package.json

EXPOSE 3000

# Note: entrypoint runs migrations which may need write access,
# but the migration tool writes to the database, not filesystem.
USER fubbik

ENTRYPOINT ["sh", "entrypoint.sh"]
