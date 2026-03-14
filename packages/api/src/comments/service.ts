import {
    listComments as listCommentsRepo,
    createComment as createCommentRepo,
    updateComment as updateCommentRepo,
    deleteComment as deleteCommentRepo,
    getCommentCount as getCommentCountRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function listComments(chunkId: string) {
    return listCommentsRepo(chunkId);
}

export function createComment(chunkId: string, userId: string, content: string) {
    return createCommentRepo({
        id: crypto.randomUUID(),
        chunkId,
        userId,
        content
    });
}

export function updateComment(id: string, userId: string, content: string) {
    return updateCommentRepo(id, userId, content).pipe(
        Effect.filterOrFail(
            (comment): comment is NonNullable<typeof comment> => comment !== null,
            () => new NotFoundError({ resource: "Comment" })
        )
    );
}

export function deleteComment(id: string, userId: string) {
    return deleteCommentRepo(id, userId).pipe(
        Effect.filterOrFail(
            (comment): comment is NonNullable<typeof comment> => comment !== null,
            () => new NotFoundError({ resource: "Comment" })
        )
    );
}

export function getCommentCount(chunkId: string) {
    return getCommentCountRepo(chunkId);
}
