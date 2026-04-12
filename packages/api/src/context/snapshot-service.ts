import {
    createSnapshot as createSnapshotRepo,
    deleteSnapshot as deleteSnapshotRepo,
    getSnapshotById,
    listSnapshots as listSnapshotsRepo,
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";
import { formatStructuredMarkdown, formatStructured } from "./formatter";
import { enrichChunks, resolveForConcept, resolveForFiles, resolveForPlan } from "./resolvers";
import { budgetChunks, estimateTokens } from "./utils";

const DEFAULT_MAX_TOKENS = 8000;

export interface SnapshotInput {
    planId?: string;
    taskId?: string;
    filePaths?: string[];
    concept?: string;
    maxTokens?: number;
    codebaseId?: string;
}

export function createSnapshot(userId: string, input: SnapshotInput) {
    return Effect.gen(function* () {
        const maxTokens = input.maxTokens ?? DEFAULT_MAX_TOKENS;

        // Resolve chunk IDs based on input type
        let chunkIds: string[] = [];

        if (input.planId) {
            chunkIds = yield* resolveForPlan(input.planId);
        } else if (input.filePaths && input.filePaths.length > 0) {
            chunkIds = yield* resolveForFiles(input.filePaths, userId, input.codebaseId);
        } else if (input.concept) {
            chunkIds = yield* resolveForConcept(input.concept, userId, input.codebaseId);
        }

        const enriched = yield* enrichChunks(chunkIds, userId);
        const budgeted = budgetChunks(enriched, maxTokens);
        const structured = formatStructured(budgeted);
        const content = formatStructuredMarkdown(structured);
        const tokenCount = estimateTokens(content);

        const query: Record<string, unknown> = {};
        if (input.planId) query.planId = input.planId;
        if (input.taskId) query.taskId = input.taskId;
        if (input.filePaths) query.filePaths = input.filePaths;
        if (input.concept) query.concept = input.concept;
        if (input.maxTokens) query.maxTokens = input.maxTokens;
        if (input.codebaseId) query.codebaseId = input.codebaseId;

        const snapshot = yield* createSnapshotRepo({
            userId,
            query,
            chunks: budgeted,
            tokenCount,
        });

        return {
            snapshotId: snapshot.id,
            tokenCount: snapshot.tokenCount,
            chunkCount: budgeted.length,
            createdAt: snapshot.createdAt,
        };
    });
}

export function getSnapshot(id: string) {
    return Effect.gen(function* () {
        const snapshot = yield* getSnapshotById(id);
        if (!snapshot) {
            return yield* Effect.fail(new NotFoundError({ resource: "ContextSnapshot" }));
        }
        return snapshot;
    });
}

export function listSnapshots(userId: string) {
    return listSnapshotsRepo(userId);
}

export function deleteSnapshot(id: string) {
    return deleteSnapshotRepo(id);
}
