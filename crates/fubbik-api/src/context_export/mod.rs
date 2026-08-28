//! `GET /api/chunks/export/context` and `GET /api/chunks/export/claude-md`.
//!
//! Ports `packages/api/src/context-export/service.ts` (token-budgeted chunk
//! export, optionally boosted toward a `forPath`) and
//! `packages/api/src/context-export/claude-md.ts` (tag-based export of
//! chunks plus requirements and in-progress plans, defaulting to a
//! 32000-token budget). Both reuse the enrichment/scoring/budgeting
//! machinery Tasks 5-6 built for `/api/context/*` rather than duplicating
//! it — `service::export_context` calls straight into
//! `context::service::enrich_chunks` and `context::routes::budget_metadata`,
//! the same order-preserving budgeting tail `for-plan`/`about`/`for-files`
//! use.

pub mod claude_md;
pub mod routes;
pub mod service;
