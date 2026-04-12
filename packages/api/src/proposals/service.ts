import {
    createProposal as createProposalRepo,
    getProposalById,
    getPendingCount as getPendingCountRepo,
    listProposals as listProposalsRepo,
    listProposalsForChunk as listProposalsForChunkRepo,
    updateProposalStatus,
} from "@fubbik/db/repository";
import type { ProposedChanges } from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";
import { updateChunk } from "../chunks/service";

export function createProposal(
    chunkId: string,
    proposedBy: string,
    body: { changes: ProposedChanges; reason?: string }
) {
    if (!body.changes || Object.keys(body.changes).length === 0) {
        return Effect.fail(new ValidationError({ message: "changes must not be empty" }));
    }
    return createProposalRepo({
        id: crypto.randomUUID(),
        chunkId,
        proposedBy,
        changes: body.changes,
        reason: body.reason ?? null,
        status: "pending",
    });
}

export function getProposal(proposalId: string) {
    return getProposalById(proposalId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Proposal" }))
        )
    );
}

export function listProposals(filter: {
    chunkId?: string;
    status?: string;
    limit?: number;
    offset?: number;
}) {
    const validStatuses = ["pending", "approved", "rejected"];
    if (filter.status && !validStatuses.includes(filter.status)) {
        return Effect.fail(new ValidationError({ message: `status must be one of: ${validStatuses.join(", ")}` }));
    }
    return listProposalsRepo({
        chunkId: filter.chunkId,
        status: filter.status ?? "pending",
        limit: filter.limit,
        offset: filter.offset,
    });
}

export function listProposalsForChunk(chunkId: string, status?: string) {
    return listProposalsForChunkRepo(chunkId, status);
}

export function approveProposal(proposalId: string, reviewerId: string, note?: string) {
    return getProposalById(proposalId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Proposal" }))
        ),
        Effect.flatMap(proposal => {
            if (proposal.status !== "pending") {
                return Effect.fail(
                    new ValidationError({ message: `Proposal is already ${proposal.status}` })
                );
            }
            const changes = proposal.changes as ProposedChanges;
            return updateChunk(proposal.chunkId, reviewerId, {
                ...(changes.title !== undefined && { title: changes.title }),
                ...(changes.content !== undefined && { content: changes.content }),
                ...(changes.type !== undefined && { type: changes.type }),
                ...(changes.tags !== undefined && { tags: changes.tags }),
                ...(changes.rationale !== undefined && { rationale: changes.rationale }),
                ...(changes.alternatives !== undefined && { alternatives: changes.alternatives }),
                ...(changes.consequences !== undefined && { consequences: changes.consequences }),
                ...(changes.scope !== undefined && { scope: changes.scope }),
            }).pipe(
                Effect.flatMap(() => updateProposalStatus(proposalId, "approved", reviewerId, note))
            );
        })
    );
}

export function rejectProposal(proposalId: string, reviewerId: string, note?: string) {
    return getProposalById(proposalId).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Proposal" }))
        ),
        Effect.flatMap(proposal => {
            if (proposal.status !== "pending") {
                return Effect.fail(
                    new ValidationError({ message: `Proposal is already ${proposal.status}` })
                );
            }
            return updateProposalStatus(proposalId, "rejected", reviewerId, note);
        })
    );
}

export function bulkAction(
    actions: Array<{ proposalId: string; action: "approve" | "reject"; note?: string }>,
    reviewerId: string
) {
    return Effect.forEach(
        actions,
        ({ proposalId, action, note }) =>
            action === "approve"
                ? approveProposal(proposalId, reviewerId, note)
                : rejectProposal(proposalId, reviewerId, note),
        { concurrency: 1 }
    );
}

export function getPendingCount() {
    return getPendingCountRepo();
}
