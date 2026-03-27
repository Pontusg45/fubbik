import { api } from "./api";
import { unwrapEden } from "./eden";

// Typed helpers for common API patterns that Eden can't type correctly
// (path-parameterized sub-resources like /chunks/:id/archive)

export async function archiveChunk(id: string) {
    const { error } = await (api.api.chunks as any)[id].archive.post();
    if (error) throw new Error("Failed to archive chunk");
}

export async function restoreChunk(id: string) {
    const { error } = await (api.api.chunks as any)[id].restore.post();
    if (error) throw new Error("Failed to restore chunk");
}

export async function enrichChunk(id: string) {
    return unwrapEden(await (api.api.chunks as any)[id].enrich.post());
}

export async function getArchivedChunks() {
    return unwrapEden(await (api.api.chunks as any).archived.get());
}

export async function bulkUpdateChunks(body: { ids: string[]; data: Record<string, unknown> }) {
    const { error } = await (api.api.chunks as any)["bulk-update"].post(body);
    if (error) throw new Error("Failed to bulk update chunks");
}
