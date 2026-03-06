import { createEnv } from "@t3-oss/env-core";
import { type } from "arktype";

export const env = createEnv({
    clientPrefix: "VITE_",
    client: {
        VITE_SERVER_URL: type("string.url")
    },
    runtimeEnv: {
        VITE_SERVER_URL:
            (import.meta as any).env?.VITE_SERVER_URL ?? (typeof process !== "undefined" ? process.env.VITE_SERVER_URL : undefined)
    },
    emptyStringAsUndefined: true
});
