import { unwrapResponse } from "@fubbik/client";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

import { api } from "./api-client.js";
import { formatBoard, type BoardSnapshot } from "./coordination-format.js";
import type { McpPlugin } from "./plugin.js";

function jsonText(value: unknown) {
    return { content: [{ type: "text" as const, text: JSON.stringify(value, null, 2) }] };
}

export function registerCoordinationTools(server: McpServer): void {
    server.tool(
        "join_board",
        "Join or reconnect to a persistent Plan board. Reuse externalKey across restarts and retain the returned runId.",
        {
            planId: z.string(),
            handle: z.string(),
            parentRunId: z.string().optional(),
            externalKey: z.string().optional(),
            capabilities: z.array(z.string()).optional(),
            metadata: z.record(z.unknown()).optional()
        },
        async ({ planId, ...body }) => jsonText(unwrapResponse(await api.api.plans({ planId }).board.runs.post(body)))
    );

    server.tool(
        "read_board",
        "Read persistent tasks, claims, agents, and journal changes. Direct entries persist but do not wake recipients. Retain nextSequence for the next read.",
        {
            planId: z.string(),
            runId: z.string().optional(),
            afterSequence: z.number().int().nonnegative().optional(),
            limit: z.number().int().positive().max(500).optional()
        },
        async ({ planId, runId, afterSequence, limit }) => {
            const board: BoardSnapshot = unwrapResponse(
                await api.api.plans({ planId }).board.get({ query: { runId, afterSequence, limit } })
            );
            return { content: [{ type: "text" as const, text: formatBoard(board, runId) }] };
        }
    );

    server.tool(
        "claim_task",
        "Claim, renew, or release a task lease. Renew leases during long-running work.",
        {
            planId: z.string(),
            taskId: z.string(),
            runId: z.string(),
            action: z.enum(["claim", "renew", "release"]),
            leaseSeconds: z.number().int().min(60).max(3600).optional()
        },
        async ({ planId, taskId, ...body }) =>
            jsonText(unwrapResponse(await api.api.plans({ planId }).board.tasks({ taskId }).claim.post(body)))
    );

    server.tool(
        "update_board_task",
        "Transition a task held by this run. Reuse clientMutationId when retrying an uncertain response.",
        {
            planId: z.string(),
            taskId: z.string(),
            runId: z.string(),
            status: z.enum(["pending", "in_progress", "done", "skipped", "blocked"]),
            note: z.string().optional(),
            clientMutationId: z.string()
        },
        async ({ planId, taskId, ...body }) =>
            jsonText(unwrapResponse(await api.api.plans({ planId }).board.tasks({ taskId }).transition.post(body)))
    );

    server.tool(
        "write_board_entry",
        "Persist a board note or addressed message. Use a stable clientMutationId across retries; delivery does not wake another agent.",
        {
            planId: z.string(),
            runId: z.string(),
            kind: z.enum(["note", "question", "answer", "progress", "decision", "handoff", "artifact", "system"]),
            body: z.string(),
            clientMutationId: z.string(),
            taskId: z.string().optional(),
            recipientRunId: z.string().optional(),
            replyToId: z.string().optional(),
            metadata: z.record(z.unknown()).optional()
        },
        async ({ planId, ...body }) => jsonText(unwrapResponse(await api.api.plans({ planId }).board.entries.post(body)))
    );

    server.tool(
        "ack_board",
        "Persist this run's journal cursor and heartbeat after processing entries.",
        {
            planId: z.string(),
            runId: z.string(),
            throughSequence: z.number().int().nonnegative(),
            status: z.enum(["active", "finished", "abandoned"]).optional()
        },
        async ({ planId, runId, ...body }) => jsonText(unwrapResponse(await api.api.plans({ planId }).board.runs({ runId }).ack.post(body)))
    );
}

export const coordinationPlugin: McpPlugin = {
    name: "coordination",
    description: "Persistent Plan taskboard and journal tools for collaborating agents",
    register: registerCoordinationTools
};
