import type { CoordinationBoard } from "@fubbik/client";

export function formatBoard(board: CoordinationBoard, runId?: string): string {
    const runById = new Map(board.runs.map(run => [run.id, run]));
    const claimByTask = new Map(board.claims.map(claim => [claim.taskId, claim]));
    const lines = [`# ${board.plan.title}`, `Plan: ${board.plan.id} (${board.plan.status})`, "", "## Tasks"];

    if (board.tasks.length === 0) lines.push("No tasks.");
    for (const task of board.tasks) {
        const claim = claimByTask.get(task.id);
        const holder = claim ? (runById.get(claim.agentRunId)?.handle ?? claim.agentRunId) : undefined;
        const lease = claim ? `${claim.expired ? "expired" : "until"} ${claim.leaseExpiresAt}` : "unclaimed";
        lines.push(`- [${task.status}] ${task.title} (${task.id}) — ${holder ? `${holder}, ${lease}` : lease}`);
    }

    lines.push("", "## Agents");
    if (board.runs.length === 0) lines.push("No agent runs.");
    for (const run of board.runs) {
        const parent = run.parentRunId ? `, parent ${run.parentRunId}` : "";
        lines.push(`- ${run.handle} [${run.status}] (${run.id}${parent})`);
    }

    const direct = runId ? board.entries.filter(entry => entry.recipientRunId === runId) : [];
    lines.push("", "## Direct messages");
    if (direct.length === 0) lines.push("No new direct messages.");
    for (const entry of direct) {
        lines.push(`- #${entry.sequence} ${runById.get(entry.authorRunId)?.handle ?? entry.authorRunId}: ${entry.body}`);
    }

    lines.push("", "## Journal");
    if (board.entries.length === 0) lines.push("No new entries.");
    for (const entry of board.entries) {
        const scope = entry.taskId ? ` task ${entry.taskId}` : "";
        const recipient = entry.recipientRunId ? ` → ${entry.recipientRunId}` : "";
        lines.push(`- #${entry.sequence} [${entry.kind}] ${entry.authorRunId}${recipient}${scope}: ${entry.body}`);
    }

    lines.push("", `Next cursor: ${board.cursor.nextSequence}`);
    if (board.cursor.hasMore) lines.push("More entries are available; read again from the next cursor.");
    return lines.join("\n");
}
