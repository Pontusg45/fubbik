import { describe, expect, it } from "vitest";

import { buildAgentCoordinationFixtures } from "./agent-coordination";

const taskIds = {
    schema: "task-schema",
    uiCatalog: "task-ui-catalog",
    fkCutover: "task-fk-cutover",
    crudUi: "task-crud-ui",
    inverseUi: "task-inverse-ui"
};

describe("agent coordination seed fixtures", () => {
    it("builds a coherent multi-agent board with representative coordination data", () => {
        let nextId = 0;
        const now = new Date("2026-09-04T12:00:00.000Z");
        const fixtures = buildAgentCoordinationFixtures({
            planId: "plan-vocab",
            taskIds,
            now,
            id: () => `seed-id-${++nextId}`
        });

        expect(fixtures.runs.map(run => [run.handle, run.status])).toEqual([
            ["coordinator", "active"],
            ["ui-implementer", "active"],
            ["schema-reviewer", "finished"]
        ]);
        expect(fixtures.runs[1]?.parentRunId).toBe(fixtures.runs[0]?.id);
        expect(fixtures.runs[2]?.parentRunId).toBe(fixtures.runs[0]?.id);

        expect(fixtures.claim).toMatchObject({
            planId: "plan-vocab",
            taskId: taskIds.inverseUi,
            agentRunId: fixtures.runs[1]?.id
        });
        expect(fixtures.claim.leaseExpiresAt.getTime()).toBeGreaterThan(now.getTime());

        expect(new Set(fixtures.entries.map(entry => entry.kind))).toEqual(
            new Set(["system", "artifact", "decision", "handoff", "progress", "question", "answer"])
        );
        expect(fixtures.entries.some(entry => entry.recipientRunId !== null)).toBe(true);
        expect(fixtures.entries.some(entry => entry.replyToId !== null)).toBe(true);
        expect(fixtures.entries.every(entry => entry.planId === "plan-vocab")).toBe(true);
        expect(new Set(fixtures.entries.map(entry => entry.clientMutationId)).size).toBe(fixtures.entries.length);
    });
});
