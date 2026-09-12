# ADR 0003: Background work is owned by a server instance

- Status: Accepted
- Date: 2026-09-12

## Context

The background task tracker and cancellation token are process-global. Once
shutdown closes that tracker, another server instance in the same process
cannot obtain an independent lifecycle, and tests cannot reliably isolate or
drain spawned work.

## Decision

Each server instance owns a background runtime. The runtime supervises
recurring jobs and detached request work, exposes cancellation and spawning to
the server implementation, and drains during that instance's shutdown.

## Consequences

- The runtime handle travels through application state.
- Tests can create, await, and stop isolated runtimes.
- No process-global reset interface is required.

