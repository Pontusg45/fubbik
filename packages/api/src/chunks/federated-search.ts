import { listChunksWithCodebase } from "@fubbik/db/repository";

export function federatedSearch(userId: string, query: {
    search?: string;
    type?: string;
    tags?: string;
    limit?: string;
    offset?: string;
    sort?: string;
}) {
    return listChunksWithCodebase({
        userId,
        search: query.search,
        type: query.type,
        tags: query.tags ? query.tags.split(",") : undefined,
        sort: (query.sort as "newest" | "oldest" | "alpha" | "updated") ?? "updated",
        limit: Math.min(Number(query.limit) || 20, 50),
        offset: Number(query.offset) || 0,
    });
}
