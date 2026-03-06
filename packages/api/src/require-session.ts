import { Effect } from "effect";

import type { Session } from "./context";
import { AuthError } from "./errors";

export function requireSession(ctx: unknown): Effect.Effect<NonNullable<Session>, AuthError> {
    const session = (ctx as unknown as { session: Session }).session;
    if (!session) return Effect.fail(new AuthError());
    return Effect.succeed(session);
}
