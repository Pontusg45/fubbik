import assert from "node:assert/strict";
import test from "node:test";

import type { CoordinationBoard } from "@fubbik/client";

import { formatBoard } from "./coordination-format.js";

test("formats full identifiers, claims, direct messages, and pagination", () => {
    // Given
    const board: CoordinationBoard = {
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
                body: "Done",
                createdAt: "2026-09-04T11:59:00.000Z"
            }
        ],
        cursor: { nextSequence: 42, acknowledgedSequence: 1, hasMore: true }
    };
    // When
    const text = formatBoard(board, "root-full-id");
    for (const expected of ["plan-full-id", "task-full-id", "child-full-id", "#42", "Done", "More entries are available"]) {
        // Then
        assert.match(text, new RegExp(expected));
    }
});
