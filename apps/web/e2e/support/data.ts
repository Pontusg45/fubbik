import { randomUUID } from "node:crypto";

import type { Registration } from "./screens/auth";

export function testAccount(overrides: Partial<Registration> = {}): Registration {
    return {
        name: "Test User",
        email: `e2e-${randomUUID()}@example.com`,
        password: "testpassword123",
        ...overrides
    };
}
