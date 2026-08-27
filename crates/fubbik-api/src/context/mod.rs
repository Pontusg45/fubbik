//! Context resolution: turns an input (a plan, a concept, a set of file
//! paths) into candidate chunk ids, then enriches those ids into fully
//! scored, health-annotated chunks. Ports `packages/api/src/context/
//! resolvers.ts`.
//!
//! [`resolvers`] produces **candidate ids only**; [`service::enrich_chunks`]
//! is the separate enrichment step that fetches full rows, connections,
//! tags, staleness/proposal flags and active-feature overlays, and scores
//! the result. That split — resolve, then enrich — is what lets plan,
//! concept and file inputs share one enrichment pipeline instead of three
//! (see `resolvers`'s module doc for the one deliberate divergence from
//! Node in the plan resolver's ownership handling).

pub mod dto;
pub mod resolvers;
pub mod service;
