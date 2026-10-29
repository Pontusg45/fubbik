import { defineConfig } from "vitest/config";

// Set env vars before any module imports (dotenv/config in env package runs at import time)
process.env.NODE_ENV = "test";
process.env.DATABASE_URL = process.env.DATABASE_URL ?? "postgresql://pontus@localhost:5432/fubbik";
process.env.BETTER_AUTH_SECRET = "test-secret-that-is-at-least-32-characters-long";
process.env.BETTER_AUTH_URL = "http://localhost:3000";
process.env.CORS_ORIGIN = "http://localhost:5173";
process.env.PORT = "3000";

export default defineConfig({
    test: {
        environment: "node",
        // Tokenizer initialization can exceed Vitest's 5s default when the
        // monorepo test task runs every package concurrently in CI.
        testTimeout: 15_000
    }
});
