import { auth } from "@fubbik/auth";
import { Cause, Effect, Option } from "effect";
import { Elysia } from "elysia";

import { aiRoutes } from "./ai/routes";
import { appliesToRoutes } from "./applies-to/routes";
import { chunkRoutes } from "./chunks/routes";
import { commentRoutes } from "./comments/routes";
import { coverageRoutes } from "./coverage/routes";
import { diagramRoutes } from "./diagram/routes";
import { sessionRoutes } from "./sessions/routes";
import { codebaseRoutes } from "./codebases/routes";
import { collectionRoutes } from "./collections/routes";
import { connectionRoutes } from "./connections/routes";
import { contextExportRoutes } from "./context-export/routes";
import { contextForFileRoutes } from "./context-for-file/routes";
import { favoriteRoutes } from "./favorites/routes";
import { generateInstructionsRoutes } from "./generate-instructions/routes";
import { knowledgeHealthRoutes } from "./knowledge-health/routes";
import { requirementRoutes } from "./requirements/routes";
import { dependencyRoutes } from "./requirements/dependency-routes";
import type { Session } from "./context";
import { enrichRoutes } from "./enrich/routes";
import { fileRefRoutes } from "./file-refs/routes";
import { graphRoutes } from "./graph/routes";
import { healthRoutes } from "./health/routes";
import { requireSession } from "./require-session";
import { statsRoutes } from "./stats/routes";
import { tagTypeRoutes } from "./tag-types/routes";
import { tagRoutes } from "./tags/routes";
import { templateRoutes } from "./templates/routes";
import { useCaseRoutes } from "./use-cases/routes";
import { activityRoutes } from "./activity/routes";
import { notificationRoutes } from "./notifications/routes";
import { settingsRoutes } from "./settings/routes";
import { planRoutes } from "./plans/routes";
import { vocabularyRoutes } from "./vocabulary/routes";
import { workspaceRoutes } from "./workspaces/routes";

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
                case "StepValidationError":
                    set.status = 400;
                    return { message: "Invalid steps", errors: effectError.errors };
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
    .use(contextExportRoutes)
    .use(contextForFileRoutes)
    .use(chunkRoutes)
    .use(appliesToRoutes)
    .use(statsRoutes)
    .use(connectionRoutes)
    .use(graphRoutes)
    .use(aiRoutes)
    .use(enrichRoutes)
    .use(tagRoutes)
    .use(tagTypeRoutes)
    .use(codebaseRoutes)
    .use(generateInstructionsRoutes)
    .use(fileRefRoutes)
    .use(templateRoutes)
    .use(knowledgeHealthRoutes)
    .use(requirementRoutes)
    .use(dependencyRoutes)
    .use(useCaseRoutes)
    .use(vocabularyRoutes)
    .use(favoriteRoutes)
    .use(collectionRoutes)
    .use(notificationRoutes)
    .use(activityRoutes)
    .use(settingsRoutes)
    .use(commentRoutes)
    .use(coverageRoutes)
    .use(sessionRoutes)
    .use(diagramRoutes)
    .use(planRoutes)
    .use(workspaceRoutes);

export type Api = typeof api;
