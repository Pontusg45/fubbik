/**
 * Routes that are not yet in Rust's OpenAPI spec. Call sites should import
 * `legacyApi` only for these domains until they are ported.
 *
 * CI: `pnpm --filter web run check:legacy-api` fails on new `legacyApi` imports
 * outside this allowlist and the files listed below.
 */
export const NODE_ONLY_API_PREFIXES = ["ai", "chunks/grouped", "chunks/import-docs"] as const;

export type NodeOnlyApiPrefix = (typeof NODE_ONLY_API_PREFIXES)[number];

/** Files allowed to import `legacyApi` (grep-checked in `scripts/check-legacy-api.mjs`). */
export const LEGACY_API_CALL_SITES = [
    "routes/chunks.new.tsx",
    "routes/requirements_.new.tsx",
    "features/import/steps/preview.tsx",
    "features/import/quick-mode.tsx",
    "features/import/import-dialog.tsx",
    "features/import/use-sse-import.ts",
    "features/chunks/use-chunks-data.ts",
    "features/chunks/lazy-group-list.tsx",
    "features/chunks/ai-section.tsx"
] as const;
