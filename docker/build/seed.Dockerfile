# syntax=docker/dockerfile:1
# Optional sample-data tooling. This image is deliberately separate from the
# Rust API runtime so production requests never depend on Node or Bun.

FROM node:22-slim AS dependencies

RUN corepack enable && corepack prepare pnpm@10.10.0 --activate

WORKDIR /app

ENV CI=1
ENV DOTENV_DISABLE=1
ENV PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/cli/package.json ./apps/cli/
COPY apps/web/package.json ./apps/web/
COPY apps/vscode/package.json ./apps/vscode/
COPY packages/client/package.json ./packages/client/
COPY packages/config/package.json ./packages/config/
COPY packages/db/package.json ./packages/db/
COPY packages/env/package.json ./packages/env/
COPY packages/mcp/package.json ./packages/mcp/

RUN --mount=type=cache,id=fubbik-pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --prod --filter @fubbik/db...

FROM oven/bun:1.3.10-slim

WORKDIR /app

ENV DOTENV_DISABLE=1

COPY --from=dependencies /app/node_modules ./node_modules
COPY --from=dependencies /app/packages/db/node_modules ./packages/db/node_modules
COPY --from=dependencies /app/packages/env/node_modules ./packages/env/node_modules
COPY packages/config/ ./packages/config/
COPY packages/env/ ./packages/env/
COPY packages/db/ ./packages/db/

CMD ["bun", "run", "packages/db/src/seed/index.ts", "--quiet"]
