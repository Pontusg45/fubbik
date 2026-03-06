import { createEnv } from "@t3-oss/env-core";
import { type } from "arktype";

export const env = createEnv({
    clientPrefix: "EXPO_PUBLIC_",
    client: {
        EXPO_PUBLIC_SERVER_URL: type("string.url")
    },
    runtimeEnv: process.env,
    emptyStringAsUndefined: true
});
