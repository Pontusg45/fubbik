import { treaty } from "@elysiajs/eden";
import type { Api } from "@fubbik/api";
import { env } from "@fubbik/env/web";

export const api = treaty<Api>(env.VITE_SERVER_URL, {
    fetch: { credentials: "include" }
});
