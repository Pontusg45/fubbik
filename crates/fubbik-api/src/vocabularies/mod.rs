//! Port of `packages/api/src/vocabularies/routes.ts` — the two vocabulary
//! *catalogs*: `chunk_type` (`/api/chunk-types`) and `connection_relation`
//! (`/api/connection-relations`).
//!
//! Distinct from the `crate::vocabulary` module, which ports the unrelated
//! `packages/api/src/vocabulary/` directory (BDD step vocabulary entries).
//! Node keeps them in two sibling directories with confusingly similar
//! names; this crate mirrors that split rather than merging them.

pub mod dto;
pub mod routes;
pub mod service;
