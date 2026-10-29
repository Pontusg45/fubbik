import {
    createProposal as createProposalRepo,
    getProposalById,
    getPendingCount as getPendingCountRepo,
    listProposals as listProposalsRepo,
    listProposalsForChunk as listProposalsForChunkRepo,
    updateProposalStatus
} from "@fubbik/db/repository";
import type { ProposedChanges } from "@fubbik/db/repository";
import { Effect } from "effect";

import { updateChunk } from "../chunks/service";
import { NotFoundError, ValidationError } from "../errors";

export function createProposal(chunkId: string, proposedBy: string, body: { changes: ProposedChanges; reason?: string }) {
    return Effect.gen(function* () {
        if (!body.changes || Object.keys(body.changes).length === 0) {
            return yield* Effect.fail(new ValidationError({ message: "changes must not be empty" }));
        }
        return yield* createProposalRepo({
            id: crypto.randomUUID(),
            chunkId,
            proposedBy,
            changes: body.changes,
            reason: body.reason ?? null,
            status: "pending"
        });
    });
}

export function getProposal(proposalId: string, userId: string) {
    return getProposalById(proposalId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Proposal" }))))
    );
}

export function listProposals(filter: { userId: string; chunkId?: string; status?: string; limit?: number; offset?: number }) {
    return Effect.gen(function* () {
        const validStatuses = ["pending", "approved", "rejected"];
        if (filter.status && !validStatuses.includes(filter.status)) {
            return yield* Effect.fail(new ValidationError({ message: `status must be one of: ${validStatuses.join(", ")}` }));
        }
        return yield* listProposalsRepo({
            userId: filter.userId,
            chunkId: filter.chunkId,
            status: filter.status ?? "pending",
            limit: filter.limit,
            offset: filter.offset
        });
    });
}

export function listProposalsForChunk(chunkId: string, userId: string, status?: string) {
    return listProposalsForChunkRepo(chunkId, userId, status);
}

/**
 * SECURITY: the lookup is scoped by `reviewerId`, so only the owner of the
 * chunk a proposal targets can approve it.
 *
 * This did NOT show up in the session-discarding sweep, because the route
 * does read `session.user.id` — it just used it as a value to stamp
 * (`reviewedBy`) rather than as a filter. The proposal itself was fetched
 * unscoped, so any authenticated user could approve any proposal, and
 * approving WRITES the proposed changes into someone else's chunk.
 *
 * `updateChunk(proposal.chunkId, reviewerId, ...)` below is itself scoped,
 * so the write would have failed — but only after `getProposalById` had
 * already disclosed the proposal, and `rejectProposal` (which does not call
 * updateChunk) had no such backstop at all.
 */
export function approveProposal(proposalId: string, reviewerId: string, note?: string) {
    return getProposalById(proposalId, reviewerId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Proposal" })))),
        Effect.flatMap(proposal =>
            proposal.status !== "pending"
                ? Effect.fail(new ValidationError({ message: `Proposal is already ${proposal.status}` }))
                : Effect.succeed(proposal)
        ),
        Effect.flatMap(proposal => {
            const changes = proposal.changes;
            return updateChunk(proposal.chunkId, reviewerId, {
                ...(changes.title !== undefined && { title: changes.title }),
                ...(changes.content !== undefined && { content: changes.content }),
                ...(changes.type !== undefined && { type: changes.type }),
                ...(changes.tags !== undefined && { tags: changes.tags }),
                ...(changes.rationale !== undefined && { rationale: changes.rationale }),
                ...(changes.alternatives !== undefined && { alternatives: changes.alternatives }),
                ...(changes.consequences !== undefined && { consequences: changes.consequences }),
                ...(changes.scope !== undefined && { scope: changes.scope })
            }).pipe(Effect.flatMap(() => updateProposalStatus(proposalId, "approved", reviewerId, note)));
        })
    );
}

/** SECURITY: scoped by `reviewerId` — see `approveProposal`. Rejecting had
 *  no backstop at all: it never touches the chunk, so an unscoped lookup was
 *  the only thing standing between a caller and rejecting a stranger's
 *  pending proposal. */
export function rejectProposal(proposalId: string, reviewerId: string, note?: string) {
    return getProposalById(proposalId, reviewerId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Proposal" })))),
        Effect.flatMap(proposal =>
            proposal.status !== "pending"
                ? Effect.fail(new ValidationError({ message: `Proposal is already ${proposal.status}` }))
                : Effect.succeed(proposal)
        ),
        Effect.flatMap(() => updateProposalStatus(proposalId, "rejected", reviewerId, note))
    );
}

export function bulkAction(actions: Array<{ proposalId: string; action: "approve" | "reject"; note?: string }>, reviewerId: string) {
    return Effect.forEach(
        actions,
        ({ proposalId, action, note }) =>
            action === "approve" ? approveProposal(proposalId, reviewerId, note) : rejectProposal(proposalId, reviewerId, note),
        { concurrency: 1 }
    );
}

export function getPendingCount(userId: string) {
    return getPendingCountRepo(userId);
}
