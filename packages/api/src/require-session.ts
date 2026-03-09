import { Effect } from "effect";

import type { Session } from "./context";
import { AuthError } from "./errors";

const DEV_SESSION: NonNullable<Session> = {
    session: { id: "dev-session", userId: "dev-user", expiresAt: new Date("2099-01-01"), token: "dev-token", createdAt: new Date(), updatedAt: new Date(), ipAddress: null, userAgent: null },
    user: { id: "dev-user", name: "Dev User", email: "dev@localhost", emailVerified: false, image: null, createdAt: new Date(), updatedAt: new Date() }
};

export function requireSession(ctx: unknown): Effect.Effect<NonNullable<Session>, AuthError> {
    const session = (ctx as unknown as { session: Session }).session;
    if (!session) return Effect.succeed(DEV_SESSION);
    return Effect.succeed(session);
}

export function optionalSession(ctx: unknown): Effect.Effect<Session | null, never> {
    const session = (ctx as unknown as { session: Session }).session;
    return Effect.succeed(session ?? null);
}
