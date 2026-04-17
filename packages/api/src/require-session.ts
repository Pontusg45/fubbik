import { Effect } from "effect";

import type { Session } from "./context";
import { AuthError } from "./errors";

/**
 * Requires a session on the request context. Dev-mode DEV_SESSION injection
 * happens upstream in `index.ts` — by the time requireSession runs, either a
 * real session is present or the caller is genuinely unauthenticated. No
 * silent fallback here (the old version masked auth-guard test failures and
 * made it hard to distinguish "dev convenience" from "actual unauthenticated
 * request").
 */
export function requireSession(ctx: unknown): Effect.Effect<NonNullable<Session>, AuthError> {
    const session = (ctx as unknown as { session: Session }).session;
    if (!session) return Effect.fail(new AuthError({}));
    return Effect.succeed(session);
}

export function optionalSession(ctx: unknown): Effect.Effect<Session | null, never> {
    const session = (ctx as unknown as { session: Session }).session;
    return Effect.succeed(session ?? null);
}
