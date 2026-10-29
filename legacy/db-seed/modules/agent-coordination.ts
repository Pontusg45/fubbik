import { sql } from "drizzle-orm";

import { agentRun, coordinationEntry, planTaskClaim } from "../../schema/coordination";
import type { SeedContext } from "../context";
import { uuid } from "../factories";

interface CoordinationTaskIds {
    schema: string;
    uiCatalog: string;
    fkCutover: string;
    crudUi: string;
    inverseUi: string;
}

interface BuildCoordinationFixturesInput {
    planId: string;
    taskIds: CoordinationTaskIds;
    now?: Date;
    id?: () => string;
}

type AgentRunInsert = typeof agentRun.$inferInsert;
type ClaimInsert = typeof planTaskClaim.$inferInsert;
type EntryInsert = typeof coordinationEntry.$inferInsert;

export interface AgentCoordinationFixtures {
    runs: AgentRunInsert[];
    claim: ClaimInsert;
    entries: EntryInsert[];
}

/** Build a small but connected board story without touching the database. */
export function buildAgentCoordinationFixtures(input: BuildCoordinationFixturesInput): AgentCoordinationFixtures {
    const now = input.now ?? new Date();
    const nextId = input.id ?? uuid;
    const atMinutesAgo = (minutes: number) => new Date(now.getTime() - minutes * 60_000);

    const rootId = nextId();
    const implementerId = nextId();
    const reviewerId = nextId();
    const runs: AgentRunInsert[] = [
        {
            id: rootId,
            planId: input.planId,
            parentRunId: null,
            handle: "coordinator",
            externalKey: "seed/vocab-crud/coordinator",
            status: "active",
            capabilities: ["delegate", "review", "synthesize"],
            metadata: { host: "seed", role: "root" },
            createdAt: atMinutesAgo(90),
            updatedAt: atMinutesAgo(4),
            lastHeartbeatAt: atMinutesAgo(4)
        },
        {
            id: implementerId,
            planId: input.planId,
            parentRunId: rootId,
            handle: "ui-implementer",
            externalKey: "seed/vocab-crud/ui-implementer",
            status: "active",
            capabilities: ["react", "accessibility", "integration-tests"],
            metadata: { host: "seed", role: "sub-agent" },
            createdAt: atMinutesAgo(72),
            updatedAt: atMinutesAgo(2),
            lastHeartbeatAt: atMinutesAgo(2)
        },
        {
            id: reviewerId,
            planId: input.planId,
            parentRunId: rootId,
            handle: "schema-reviewer",
            externalKey: "seed/vocab-crud/schema-reviewer",
            status: "finished",
            capabilities: ["postgres", "drizzle", "review"],
            metadata: { host: "seed", role: "sub-agent" },
            createdAt: atMinutesAgo(80),
            updatedAt: atMinutesAgo(28),
            lastHeartbeatAt: atMinutesAgo(28)
        }
    ];

    const kickoffId = nextId();
    const artifactId = nextId();
    const decisionId = nextId();
    const handoffId = nextId();
    const progressId = nextId();
    const questionId = nextId();
    const answerId = nextId();
    const entries: EntryInsert[] = [
        {
            id: kickoffId,
            planId: input.planId,
            taskId: input.taskIds.schema,
            authorRunId: rootId,
            recipientRunId: null,
            replyToId: null,
            kind: "system",
            body: "Split the catalog work into schema review and UI implementation tracks.",
            metadata: { phase: "kickoff" },
            clientMutationId: "seed-coordination-kickoff",
            createdAt: atMinutesAgo(88)
        },
        {
            id: artifactId,
            planId: input.planId,
            taskId: input.taskIds.fkCutover,
            authorRunId: reviewerId,
            recipientRunId: null,
            replyToId: null,
            kind: "artifact",
            body: "Schema review complete: catalog foreign keys and cascade behavior are verified.",
            metadata: { artifact: "schema-review.md", verdict: "approved" },
            clientMutationId: "seed-schema-review-artifact",
            createdAt: atMinutesAgo(34)
        },
        {
            id: decisionId,
            planId: input.planId,
            taskId: input.taskIds.uiCatalog,
            authorRunId: rootId,
            recipientRunId: null,
            replyToId: null,
            kind: "decision",
            body: "Keep catalog slugs stable and use labels only for presentation.",
            metadata: { decision: "stable-catalog-slugs" },
            clientMutationId: "seed-catalog-slug-decision",
            createdAt: atMinutesAgo(31)
        },
        {
            id: handoffId,
            planId: input.planId,
            taskId: input.taskIds.crudUi,
            authorRunId: reviewerId,
            recipientRunId: implementerId,
            replyToId: null,
            kind: "handoff",
            body: "The schema path is clear. Continue with inverse labels and preserve the catalog IDs on writes.",
            metadata: { fromDiscipline: "schema", toDiscipline: "frontend" },
            clientMutationId: "seed-schema-to-ui-handoff",
            createdAt: atMinutesAgo(27)
        },
        {
            id: progressId,
            planId: input.planId,
            taskId: input.taskIds.inverseUi,
            authorRunId: implementerId,
            recipientRunId: null,
            replyToId: null,
            kind: "progress",
            body: "Inverse labels render on chunk detail; integration coverage is the remaining work.",
            metadata: { percent: 75 },
            clientMutationId: "seed-inverse-label-progress",
            createdAt: atMinutesAgo(12)
        },
        {
            id: questionId,
            planId: input.planId,
            taskId: input.taskIds.inverseUi,
            authorRunId: implementerId,
            recipientRunId: rootId,
            replyToId: null,
            kind: "question",
            body: "Should the fallback show the relation slug when an inverse label is absent?",
            metadata: {},
            clientMutationId: "seed-inverse-label-question",
            createdAt: atMinutesAgo(8)
        },
        {
            id: answerId,
            planId: input.planId,
            taskId: input.taskIds.inverseUi,
            authorRunId: rootId,
            recipientRunId: implementerId,
            replyToId: questionId,
            kind: "answer",
            body: "Yes. Prefer the inverse label, then fall back to a humanized relation slug.",
            metadata: {},
            clientMutationId: "seed-inverse-label-answer",
            createdAt: atMinutesAgo(5)
        }
    ];

    return {
        runs,
        claim: {
            taskId: input.taskIds.inverseUi,
            planId: input.planId,
            agentRunId: implementerId,
            claimedAt: atMinutesAgo(15),
            leaseExpiresAt: new Date(now.getTime() + 45 * 60_000),
            updatedAt: atMinutesAgo(2)
        },
        entries
    };
}

function requiredId(ids: Record<string, string>, key: string, domain: string): string {
    const id = ids[key];
    if (!id) throw new Error(`agent-coordination needs ${domain} "${key}"`);
    return id;
}

export async function seed(ctx: SeedContext): Promise<void> {
    const fixtures = buildAgentCoordinationFixtures({
        planId: requiredId(ctx.ids.plans, "vocab-crud", "plan"),
        taskIds: {
            schema: requiredId(ctx.ids.planTasks, "schema", "plan task"),
            uiCatalog: requiredId(ctx.ids.planTasks, "ui-catalog", "plan task"),
            fkCutover: requiredId(ctx.ids.planTasks, "fk-cutover", "plan task"),
            crudUi: requiredId(ctx.ids.planTasks, "crud-ui", "plan task"),
            inverseUi: requiredId(ctx.ids.planTasks, "inverse-ui", "plan task")
        }
    });

    await ctx.db.insert(agentRun).values(fixtures.runs[0]!);
    await ctx.db.insert(agentRun).values(fixtures.runs.slice(1));
    await ctx.db.insert(planTaskClaim).values(fixtures.claim);
    await ctx.db.insert(coordinationEntry).values(fixtures.entries);

    for (const run of fixtures.runs) ctx.ids.agentRuns[run.handle] = run.id;
    ctx.counters["agent_runs"] = fixtures.runs.length;
    ctx.counters["task_claims"] = 1;
    ctx.counters["coordination_entries"] = fixtures.entries.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.execute(sql`DELETE FROM agent_run WHERE plan_id IN (SELECT id FROM plan WHERE user_id = ${ctx.userId})`);
}
