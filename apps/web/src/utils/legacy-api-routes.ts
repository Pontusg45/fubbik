/**
 * Application routes that are not yet in Rust's OpenAPI spec. This list is
 * intentionally empty; Better Auth still targets Node through its own SDK.
 *
 * CI: `pnpm --filter web run check:legacy-api` fails on new `legacyApi` imports
 * outside this allowlist and the files listed below.
 */
export const NODE_ONLY_API_PREFIXES = [] as const;

export type NodeOnlyApiPrefix = (typeof NODE_ONLY_API_PREFIXES)[number];

/** Files allowed to import `legacyApi` (grep-checked in `scripts/check-legacy-api.mjs`). */
export const LEGACY_API_CALL_SITES = [] as const;
