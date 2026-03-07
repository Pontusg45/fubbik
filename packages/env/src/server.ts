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
        OPENAI_API_KEY: type("string | undefined"),
        OPENAI_MODEL: type("string | undefined"),
        PORT: type("string >= 1"),
        RATE_LIMIT_DURATION_MS: type("string | undefined"),
        RATE_LIMIT_MAX: type("string | undefined"),
        OLLAMA_URL: type("string | undefined")
    },
    runtimeEnv: {
        ...process.env,
        NODE_ENV: process.env.NODE_ENV ?? "development",
        OPENAI_API_KEY: process.env.OPENAI_API_KEY,
        OPENAI_MODEL: process.env.OPENAI_MODEL,
        PORT: process.env.PORT ?? "3000",
        RATE_LIMIT_DURATION_MS: process.env.RATE_LIMIT_DURATION_MS,
        RATE_LIMIT_MAX: process.env.RATE_LIMIT_MAX,
        OLLAMA_URL: process.env.OLLAMA_URL
    },
    emptyStringAsUndefined: true
});
