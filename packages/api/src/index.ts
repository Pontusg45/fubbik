import { auth } from "@fubbik/auth";
import { Cause, Effect, Option } from "effect";
import { Elysia } from "elysia";

import { aiRoutes } from "./ai/routes";
import { chunkRoutes } from "./chunks/routes";
import { connectionRoutes } from "./connections/routes";
import type { Session } from "./context";
import { enrichRoutes } from "./enrich/routes";
import { graphRoutes } from "./graph/routes";
import { healthRoutes } from "./health/routes";
import { requireSession } from "./require-session";
import { statsRoutes } from "./stats/routes";
import { tagTypeRoutes } from "./tag-types/routes";
import { tagRoutes } from "./tags/routes";

const FiberFailureCauseSymbol = Symbol.for("effect/Runtime/FiberFailure/Cause");

function extractEffectError(error: unknown): Record<string, unknown> | null {
    if (typeof error !== "object" || error === null) return null;
    const cause = (error as Record<symbol, unknown>)[FiberFailureCauseSymbol];
    if (!cause) return null;
    const option = Cause.failureOption(cause as Cause.Cause<Record<string, unknown>>);
    return Option.isSome(option) ? option.value : null;
}

const isDev = process.env.NODE_ENV !== "production";

const DEV_USER_ID = "dev-user";
const DEV_SESSION: Session = {
    session: {
        id: "dev-session",
        token: "dev-token",
        userId: DEV_USER_ID,
        expiresAt: new Date(Date.now() + 86400000),
        createdAt: new Date(),
        updatedAt: new Date(),
        ipAddress: null,
        userAgent: null
    },
    user: {
        id: DEV_USER_ID,
        name: "Dev User",
        email: "dev@localhost",
        emailVerified: false,
        image: null,
        createdAt: new Date(),
        updatedAt: new Date()
    }
};

async function getSession(headers: Headers): Promise<Session> {
    try {
        const session = await auth.api.getSession({ headers });
        if (!session && isDev) return DEV_SESSION;
        return session;
    } catch {
        if (isDev) return DEV_SESSION;
        return null as unknown as Session;
    }
}

export const api = new Elysia({ prefix: "/api" })
    .use(healthRoutes)
    .onError(({ error, set }) => {
        const effectError = extractEffectError(error);
        if (effectError) {
            switch (effectError._tag) {
                case "ValidationError":
                    set.status = 400;
                    return { message: effectError.message as string };
                case "AuthError":
                    set.status = 401;
                    return { message: "Authentication required" };
                case "NotFoundError":
                    set.status = 404;
                    return { message: `${effectError.resource} not found` };
                case "AiError":
                    set.status = 502;
                    console.error("AI service error", effectError.cause);
                    return { message: "AI service error" };
                case "DatabaseError":
                    set.status = 500;
                    console.error("Database error", effectError.cause);
                    return { message: "Internal server error" };
            }
        }
    })
    .resolve(async ({ headers }) => {
        const session = await getSession(new Headers(headers as Record<string, string>));
        return { session };
    })
    .get("/me", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.map(session => ({ message: "This is private" as const, user: session.user }))))
    )
    .use(chunkRoutes)
    .use(statsRoutes)
    .use(connectionRoutes)
    .use(graphRoutes)
    .use(aiRoutes)
    .use(enrichRoutes)
    .use(tagRoutes)
    .use(tagTypeRoutes);

export type Api = typeof api;
