import { createEnv } from "@t3-oss/env-core";
import { type } from "arktype";

export const env = createEnv({
  clientPrefix: "VITE_",
  client: {
    VITE_SERVER_URL: type("string.url"),
  },
  runtimeEnv: (import.meta as any).env,
  emptyStringAsUndefined: true,
});
