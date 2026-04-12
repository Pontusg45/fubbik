import { Command } from "commander";

import { formatBold, formatDim, formatSuccess } from "../lib/colors";
import { isJson, output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

// ── Helpers ─────────────────────────────────────────────────────────

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

async function fetchApi(path: string, opts?: RequestInit): Promise<Response> {
    const serverUrl = requireServer();
    return fetch(`${serverUrl}/api${path}`, {
        ...opts,
        headers: {
            "Content-Type": "application/json",
            ...opts?.headers,
        },
    });
}

interface Proposal {
    id: string;
    chunkId: string;
    changes: Record<string, unknown>;
    reason: string | null;
    status: string;
    proposedBy: string;
    createdAt: string;
    chunkTitle?: string;
    chunkType?: string;
}

function statusIcon(status: string): string {
    switch (status) {
        case "pending":
            return "○";
        case "approved":
            return "✓";
        case "rejected":
            return "✗";
        default:
            return "·";
    }
}

// ── Subcommands ─────────────────────────────────────────────────────

const listProposals = new Command("list")
    .description("List proposals (default: pending)")
    .option("-s, --status <status>", "Filter by status (pending, approved, rejected, all)", "pending")
    .option("-c, --chunk <chunkId>", "Filter by chunk ID")
    .option("-l, --limit <n>", "Max results", "20")
    .action(async (opts: { status: string; chunk?: string; limit: string }, cmd: Command) => {
        try {
            const params = new URLSearchParams();
            if (opts.status !== "all") params.set("status", opts.status);
            if (opts.chunk) params.set("chunkId", opts.chunk);
            params.set("limit", opts.limit);

            const res = await fetchApi(`/proposals?${params.toString()}`);
            if (!res.ok) {
                outputError(`Failed to list proposals: ${res.status}`);
                process.exit(1);
            }

            const proposals = (await res.json()) as Proposal[];
            if (isJson(cmd)) {
                output(cmd, proposals, "");
                return;
            }

            if (proposals.length === 0) {
                console.log(formatDim("No proposals found."));
                return;
            }

            for (const p of proposals) {
                const icon = statusIcon(p.status);
                const fields = Object.keys(p.changes).join(", ");
                const age = timeSince(p.createdAt);
                console.log(
                    `  ${icon} ${formatBold(p.chunkTitle ?? p.chunkId.slice(0, 8))} ${formatDim(`[${fields}]`)} ${formatDim(age)} ${formatDim(p.id.slice(0, 8))}`,
                );
                if (p.reason) {
                    console.log(`    ${formatDim(p.reason)}`);
                }
            }
            console.log(formatDim(`\n${proposals.length} proposal(s)`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const showProposal = new Command("show")
    .description("Show proposal detail")
    .argument("<proposalId>", "proposal ID")
    .action(async (proposalId: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/proposals/${proposalId}`);
            if (!res.ok) {
                outputError(`Failed to get proposal: ${res.status}`);
                process.exit(1);
            }

            const proposal = (await res.json()) as Proposal;
            if (isJson(cmd)) {
                output(cmd, proposal, "");
                return;
            }

            console.log(`${formatBold("Proposal")} ${proposal.id}`);
            console.log(`${formatDim("Chunk:")} ${proposal.chunkId}`);
            console.log(`${formatDim("Status:")} ${proposal.status}`);
            console.log(`${formatDim("Proposed by:")} ${proposal.proposedBy}`);
            console.log(`${formatDim("Created:")} ${new Date(proposal.createdAt).toLocaleString()}`);
            if (proposal.reason) {
                console.log(`${formatDim("Reason:")} ${proposal.reason}`);
            }
            console.log(`${formatDim("Changes:")}`);
            for (const [field, value] of Object.entries(proposal.changes)) {
                const display =
                    typeof value === "string" && value.length > 80
                        ? `${value.slice(0, 80)}…`
                        : JSON.stringify(value);
                console.log(`  ${formatBold(field)}: ${display}`);
            }
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const approveProposal = new Command("approve")
    .description("Approve a proposal (applies changes to the chunk)")
    .argument("<proposalId>", "proposal ID")
    .option("-n, --note <note>", "Review note")
    .action(async (proposalId: string, opts: { note?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = {};
            if (opts.note) body.note = opts.note;

            const res = await fetchApi(`/proposals/${proposalId}/approve`, {
                method: "POST",
                body: JSON.stringify(body),
            });

            if (!res.ok) {
                outputError(`Failed to approve: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const proposal = (await res.json()) as Proposal;
            output(cmd, proposal, formatSuccess("Proposal approved — changes applied to chunk."));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const rejectProposal = new Command("reject")
    .description("Reject a proposal (chunk unchanged)")
    .argument("<proposalId>", "proposal ID")
    .option("-n, --note <note>", "Review note")
    .action(async (proposalId: string, opts: { note?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = {};
            if (opts.note) body.note = opts.note;

            const res = await fetchApi(`/proposals/${proposalId}/reject`, {
                method: "POST",
                body: JSON.stringify(body),
            });

            if (!res.ok) {
                outputError(`Failed to reject: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const proposal = (await res.json()) as Proposal;
            output(cmd, proposal, formatSuccess("Proposal rejected."));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const countProposals = new Command("count")
    .description("Show pending proposal count")
    .action(async (_opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi("/proposals/count");
            if (!res.ok) {
                outputError(`Failed to get count: ${res.status}`);
                process.exit(1);
            }

            const data = (await res.json()) as { pending: number };
            output(cmd, data, `${data.pending} pending proposal(s)`);
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const proposeCommand = new Command("propose")
    .description("Propose changes to a chunk (creates a pending proposal)")
    .argument("<chunkId>", "chunk ID to propose changes for")
    .option("-t, --title <title>", "Proposed new title")
    .option("-c, --content <content>", "Proposed new content")
    .option("--type <type>", "Proposed new type")
    .option("--tags <tags>", "Proposed tags (comma-separated)")
    .option("-r, --reason <reason>", "Why you're proposing this change")
    .action(async (chunkId: string, opts: { title?: string; content?: string; type?: string; tags?: string; reason?: string }, cmd: Command) => {
        try {
            const changes: Record<string, unknown> = {};
            if (opts.title) changes.title = opts.title;
            if (opts.content) changes.content = opts.content;
            if (opts.type) changes.type = opts.type;
            if (opts.tags) changes.tags = opts.tags.split(",").map(t => t.trim());

            if (Object.keys(changes).length === 0) {
                outputError("At least one change is required (--title, --content, --type, or --tags)");
                process.exit(1);
            }

            const body: Record<string, unknown> = { changes };
            if (opts.reason) body.reason = opts.reason;

            const res = await fetchApi(`/chunks/${chunkId}/proposals`, {
                method: "POST",
                body: JSON.stringify(body),
            });

            if (!res.ok) {
                outputError(`Failed to create proposal: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const proposal = (await res.json()) as Proposal;
            output(cmd, proposal, formatSuccess(`Proposal created (${proposal.id.slice(0, 8)}) — pending review.`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

// ── Helper ──────────────────────────────────────────────────────────

function timeSince(dateStr: string): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

// ── Export ───────────────────────────────────────────────────────────

export const reviewCommand = new Command("review")
    .description("Review AI-proposed chunk changes")
    .addCommand(listProposals)
    .addCommand(showProposal)
    .addCommand(approveProposal)
    .addCommand(rejectProposal)
    .addCommand(countProposals)
    .addCommand(proposeCommand);
