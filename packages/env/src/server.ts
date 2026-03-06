import "dotenv/config";
import { createEnv } from "@t3-oss/env-core";
import { type } from "arktype";

export const env = createEnv({
  server: {
    DATABASE_URL: type("string >= 1"),
    BETTER_AUTH_SECRET: type("string >= 32"),
    BETTER_AUTH_URL: type("string.url"),
    CORS_ORIGIN: type("string.url"),
    NODE_ENV: type("'development' | 'production' | 'test'"),
    PORT: type("string >= 1"),
  },
  runtimeEnv: {
    ...process.env,
    NODE_ENV: process.env.NODE_ENV ?? "development",
    PORT: process.env.PORT ?? "3000",
  },
  emptyStringAsUndefined: true,
});
