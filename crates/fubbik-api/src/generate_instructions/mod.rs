//! `GET /api/spaces/{id}/generate-instructions`.
//!
//! Ports `packages/api/src/generate-instructions/{routes,service}.ts`: lists
//! every non-archived chunk visible to the caller in the given space,
//! categorizes them by tag/type/content heuristics, and renders one of
//! three plain-text documents (`claude` → CLAUDE.md, `agents` → AGENTS.md,
//! `cursor` → .cursorrules) depending on `?format=`.
//!
//! **Both route paths are served, deliberately.** Node only ever
//! registers this handler at `/api/codebases/:id/generate-instructions`
//! (`packages/api/src/generate-instructions/routes.ts:8`) — there is no
//! `/api/spaces/:id/generate-instructions` route in Node at all, even
//! though `apps/cli/src/commands/generate.ts:35,58,81` calls exactly that
//! `/api/spaces/...` URL for all three formats. Confirmed as a live bug in
//! Node (Phase 4a's `codebases` -> `spaces` rename updated the CLI's call
//! sites but missed this one route registration), not a deliberate alias:
//! the CLI's `generate` commands 404 against a real Node server today.
//!
//! This port serves `/api/spaces/{id}/generate-instructions` as the
//! primary route — matching the CLI and the project's own rename, and
//! fixing the three broken commands — **and** keeps
//! `/api/codebases/{id}/generate-instructions` as a deprecated
//! compatibility alias pointed at the same handler, matching the path
//! Node's API genuinely still exposes today. See
//! `generate_instructions_codebases_alias_route`'s doc comment
//! (`routes.rs`) for the full reasoning.
//!
//! **Ownership check is a deliberate addition, not a port.** Node's
//! service (`service.ts:29-40`) never verifies the given `spaceId` belongs
//! to the caller — it hands `spaceId` straight to `listChunksRepo`, whose
//! space filter (ported as `chunk::push_filters`) matches chunks in that
//! space *or with no space at all*, scoped only by `chunk.user_id`. So in
//! Node, a request for someone else's space silently falls back to
//! rendering the caller's own global chunks instead of erroring — never a
//! cross-user leak (the mandatory `user_id` predicate prevents that), but
//! not a 404 either. This port instead looks the space up via
//! `space::find_by_id(pool, user_id, id)` first and returns
//! `AppError::NotFound` when it doesn't resolve (wrong owner or bogus id),
//! matching every other `/api/spaces/{id}/...` route's behaviour
//! (`spaces::service::get`) rather than silently substituting unrelated
//! content.

pub mod routes;
pub mod service;
