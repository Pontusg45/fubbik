import { createAuthClient } from "better-auth/react";

import { apiBaseUrl } from "@/lib/api-origin";

export const authClient = createAuthClient({
    baseURL: apiBaseUrl()
});
