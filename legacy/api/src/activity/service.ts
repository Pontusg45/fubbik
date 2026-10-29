import { createActivity as createActivityRepo, listActivity as listActivityRepo } from "@fubbik/db/repository";

export function listActivity(
    userId: string,
    opts: { spaceId?: string; entityType?: string; entityId?: string; limit?: number; offset?: number } = {}
) {
    return listActivityRepo(userId, opts);
}

export function createActivity(params: {
    userId: string;
    entityType: string;
    entityId: string;
    entityTitle?: string;
    action: string;
    spaceId?: string;
}) {
    return createActivityRepo({
        id: crypto.randomUUID(),
        ...params
    });
}
