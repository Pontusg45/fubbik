import { createAuthClient } from "better-auth/react";

import { legacyApiOrigin } from "@/lib/api-origin";

export const authClient = createAuthClient({
    // Better Auth still runs in the Node service. Rust verifies the session
    // cookie it issues, but does not implement Better Auth's client contract.
    baseURL: legacyApiOrigin()
});
