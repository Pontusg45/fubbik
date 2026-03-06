import { defineConfig } from "vitest/config";

// Set env vars before any module imports (dotenv/config in env package runs at import time)
process.env.NODE_ENV = "production";
process.env.DATABASE_URL = "postgresql://test:test@localhost:5432/test";
process.env.BETTER_AUTH_SECRET = "test-secret-that-is-at-least-32-characters-long";
process.env.BETTER_AUTH_URL = "http://localhost:3000";
process.env.CORS_ORIGIN = "http://localhost:5173";
process.env.PORT = "3000";

export default defineConfig({
  test: {
    environment: "node",
  },
});
