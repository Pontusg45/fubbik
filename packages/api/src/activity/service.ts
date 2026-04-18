import {
    createActivity as createActivityRepo,
    listActivity as listActivityRepo
} from "@fubbik/db/repository";

export function listActivity(
    userId: string,
    opts: { codebaseId?: string; entityType?: string; entityId?: string; limit?: number; offset?: number } = {}
) {
    return listActivityRepo(userId, opts);
}

export function createActivity(params: {
    userId: string;
    entityType: string;
    entityId: string;
    entityTitle?: string;
    action: string;
    codebaseId?: string;
}) {
    return createActivityRepo({
        id: crypto.randomUUID(),
        ...params
    });
}
