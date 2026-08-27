//! `GET /api/context/for-file` — the five-strategy chunk matcher for a
//! single file path. Ports `packages/api/src/context-for-file/`.
//!
//! [`service::get_context_for_file`] is also the delegate behind
//! `context::resolvers::resolve_for_files`, mirroring Node's own
//! `resolveForFiles`, which calls `getContextForFile` per path
//! (`resolvers.ts:220-225`) rather than reimplementing a subset of its
//! strategies.

pub mod dto;
pub mod routes;
pub mod service;
