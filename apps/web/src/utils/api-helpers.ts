import { api, legacyApi } from "./api";
import { unwrapEden } from "./eden";

// Typed helpers for common API patterns that Eden can't type correctly
// (path-parameterized sub-resources like /chunks/:id/archive)
//
// Four of the five routes here (archive/restore/archived/bulk-update) are on
// Rust now. `enrich` is not — it needs Ollama, which the Rust server does not
// call yet — so this file imports both clients.

export async function archiveChunk(id: string) {
    const { error } = await api.api.chunks({ id }).archive.post();
    if (error) throw new Error("Failed to archive chunk");
}

export async function restoreChunk(id: string) {
    const { error } = await api.api.chunks({ id }).restore.post();
    if (error) throw new Error("Failed to restore chunk");
}

export async function enrichChunk(id: string) {
    return unwrapEden(await legacyApi.api.chunks({ id }).enrich.post());
}

export async function getArchivedChunks() {
    return unwrapEden(await api.api.chunks.archived.get());
}

export async function bulkUpdateChunks(body: {
    ids: string[];
    action: "add_tags" | "remove_tags" | "set_type" | "set_codebase" | "set_review_status" | "archive" | "delete";
    value?: string | null;
}) {
    const { error } = await api.api.chunks["bulk-update"].post(body);
    if (error) throw new Error("Failed to bulk update chunks");
}
