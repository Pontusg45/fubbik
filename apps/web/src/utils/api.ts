import { apiOrigin } from "@/lib/api-origin";

import { createClient } from "./openapi-client";

export const api = createClient(apiOrigin(), { credentials: "include" });
