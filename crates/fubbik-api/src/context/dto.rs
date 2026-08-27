//! Wire DTOs for the `/api/context/*` routes.
//!
//! Deliberately empty for now. Task 5 ports only the resolver/enrichment
//! layer (`resolvers.rs`, `service.rs`) that `packages/api/src/context/
//! resolvers.ts` implements — there is no HTTP surface for `/api/context`
//! in this port yet, and `fubbik_core::format::ChunkWithMetadata` (the
//! shape `enrich_chunks` returns) already exists as the wire type
//! `format_structured` and friends serialise. This file exists so the
//! module layout matches the task brief's file list; it gains real DTOs
//! (query params, snapshot bodies) once the task that adds
//! `context::routes` wires this module into the router.
