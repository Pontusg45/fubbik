import { createEnv } from "@t3-oss/env-core";
import { type } from "arktype";

export const env = createEnv({
    clientPrefix: "VITE_",
    client: {
        /** Preferred single API origin. Leave unset in local dev for same-origin `/api` via the Vite proxy. */
        VITE_API_ORIGIN: type("string.url | undefined"),
        /** @deprecated Use `VITE_API_ORIGIN`. */
        VITE_SERVER_URL: type("string.url | undefined"),
        /** @deprecated Use `VITE_API_ORIGIN`. */
        VITE_API_URL: type("string.url | undefined"),
        /** Build-time `"true"` for Docker self-host: hide Sign In when API uses implicit dev session */
        VITE_FUBBIK_IMPLICIT_DEV_SESSION: type("string | undefined")
    },
    runtimeEnv: {
        VITE_API_ORIGIN:
            (import.meta as any).env?.VITE_API_ORIGIN ?? (typeof process !== "undefined" ? process.env.VITE_API_ORIGIN : undefined),
        VITE_SERVER_URL:
            (import.meta as any).env?.VITE_SERVER_URL ?? (typeof process !== "undefined" ? process.env.VITE_SERVER_URL : undefined),
        VITE_API_URL: (import.meta as any).env?.VITE_API_URL ?? (typeof process !== "undefined" ? process.env.VITE_API_URL : undefined),
        VITE_FUBBIK_IMPLICIT_DEV_SESSION: (import.meta as any).env?.VITE_FUBBIK_IMPLICIT_DEV_SESSION
    },
    emptyStringAsUndefined: true
});
