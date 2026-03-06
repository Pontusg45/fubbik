import { getTagsWithCounts } from "@fubbik/db/repository";

export function getUserTags(userId: string) {
    return getTagsWithCounts(userId);
}
