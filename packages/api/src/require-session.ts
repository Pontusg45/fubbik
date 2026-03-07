import { Effect } from "effect";

import type { Session } from "./context";
import { AuthError } from "./errors";

export function requireSession(ctx: unknown): Effect.Effect<NonNullable<Session>, AuthError> {
    const session = (ctx as unknown as { session: Session }).session;
    if (!session) return Effect.fail(new AuthError());
    return Effect.succeed(session);
}

export function optionalSession(ctx: unknown): Effect.Effect<Session | null, never> {
    const session = (ctx as unknown as { session: Session }).session;
    return Effect.succeed(session ?? null);
}
