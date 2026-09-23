import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PlanCoordinationPanel, type CoordinationBoard } from "./plan-coordination-panel";

describe("PlanCoordinationPanel", () => {
    it("shows agent identity, addressed journal entries, and pagination", () => {
        // Given
        const board: CoordinationBoard = {
            plan: { id: "plan-id", title: "Ship", status: "in_progress" },
            tasks: [{ id: "task-id", title: "Research", status: "done", dependsOn: [] }],
            runs: [
                { id: "root-id", parentRunId: null, handle: "root", status: "active" },
                { id: "child-id", parentRunId: "root-id", handle: "researcher", status: "finished" }
            ],
            claims: [],
            entries: [
                {
                    id: "entry-id",
                    sequence: 7,
                    taskId: "task-id",
                    authorRunId: "child-id",
                    recipientRunId: "root-id",
                    kind: "handoff",
                    body: "Research delivered",
                    createdAt: "2026-09-04T12:00:00.000Z"
                }
            ],
            cursor: { nextSequence: 7, hasMore: true }
        };

        // When
        const html = renderToStaticMarkup(<PlanCoordinationPanel board={board} isLoading={false} />);
        // Then
        expect(html).toContain("researcher");
        expect(html).toContain("handoff");
        expect(html).toContain("Research delivered");
        expect(html).toContain("More journal entries are available");
    });

    it("renders an explicit empty state", () => {
        // Given the inline inputs and test fixtures.
        // When
        const html = renderToStaticMarkup(<PlanCoordinationPanel isLoading={false} />);
        // Then
        expect(html).toContain("No agent runs yet");
        expect(html).toContain("No journal entries yet");
    });
});
