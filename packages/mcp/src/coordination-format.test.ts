import assert from "node:assert/strict";
import test from "node:test";

import { formatBoard, type BoardSnapshot } from "./coordination-format.js";

test("formats full identifiers, claims, direct messages, and pagination", () => {
    const board: BoardSnapshot = {
        plan: { id: "plan-full-id", title: "Ship", status: "in_progress" },
        tasks: [{ id: "task-full-id", title: "Research", status: "in_progress", dependsOn: [] }],
        runs: [
            { id: "root-full-id", parentRunId: null, handle: "root", status: "active", lastAckSequence: 1 },
            { id: "child-full-id", parentRunId: "root-full-id", handle: "child", status: "active", lastAckSequence: 1 }
        ],
        claims: [{ taskId: "task-full-id", agentRunId: "child-full-id", leaseExpiresAt: "2026-09-04T12:00:00.000Z", expired: false }],
        entries: [
            {
                id: "entry-full-id",
                sequence: 42,
                taskId: "task-full-id",
                authorRunId: "child-full-id",
                recipientRunId: "root-full-id",
                kind: "handoff",
                body: "Done"
            }
        ],
        cursor: { nextSequence: 42, acknowledgedSequence: 1, hasMore: true }
    };
    const text = formatBoard(board, "root-full-id");
    for (const expected of ["plan-full-id", "task-full-id", "child-full-id", "#42", "Done", "More entries are available"]) {
        assert.match(text, new RegExp(expected));
    }
});
