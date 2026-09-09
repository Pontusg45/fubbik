import type { components } from "./api-types";

type Schemas = components["schemas"];

export const coordinationRunStatuses = ["active", "finished", "abandoned"] as const;
export const coordinationEntryKinds = ["note", "question", "answer", "progress", "decision", "handoff", "artifact", "system"] as const;

export type CoordinationRunStatus = (typeof coordinationRunStatuses)[number];
export type CoordinationEntryKind = (typeof coordinationEntryKinds)[number];
export type CoordinationRun = Pick<Schemas["AgentRun"], "id" | "parentRunId" | "handle" | "status"> &
    Partial<Pick<Schemas["AgentRun"], "lastAckSequence">>;
export type CoordinationClaim = Pick<Schemas["TaskClaim"], "taskId" | "agentRunId" | "leaseExpiresAt" | "expired">;
export type CoordinationEntry = Pick<
    Schemas["CoordinationEntry"],
    "id" | "sequence" | "taskId" | "authorRunId" | "recipientRunId" | "kind" | "body" | "createdAt"
>;
export interface CoordinationBoard {
    plan: Pick<Schemas["BoardPlan"], "id" | "title" | "status">;
    tasks: Pick<Schemas["BoardTask"], "id" | "title" | "status" | "dependsOn">[];
    runs: CoordinationRun[];
    claims: CoordinationClaim[];
    entries: CoordinationEntry[];
    cursor: Pick<Schemas["BoardCursor"], "nextSequence" | "acknowledgedSequence" | "hasMore">;
}
