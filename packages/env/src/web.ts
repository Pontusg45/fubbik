import { createEnv } from "@t3-oss/env-core";
import { type } from "arktype";

export const env = createEnv({
    clientPrefix: "VITE_",
    client: {
        VITE_SERVER_URL: type("string.url"),
        /** Build-time `"true"` for Docker self-host: hide Sign In when API uses implicit dev session */
        VITE_FUBBIK_IMPLICIT_DEV_SESSION: type("string | undefined")
    },
    runtimeEnv: {
        VITE_SERVER_URL:
            (import.meta as any).env?.VITE_SERVER_URL ?? (typeof process !== "undefined" ? process.env.VITE_SERVER_URL : undefined),
        VITE_FUBBIK_IMPLICIT_DEV_SESSION: (import.meta as any).env?.VITE_FUBBIK_IMPLICIT_DEV_SESSION
    },
    emptyStringAsUndefined: true
});
