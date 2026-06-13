import { Effect } from "effect";

import { enrichChunkIfEmpty } from "../enrich/service";
import { logger } from "../logger";
import { recordUsage } from "../usage/service";
import { events, EVENTS } from "./bus";

// Auto-enrich on chunk create
events.on<{ chunkId: string; userId: string }>(EVENTS.CHUNK_CREATED, async ({ chunkId }) => {
    Effect.runPromise(enrichChunkIfEmpty(chunkId)).catch(err => {
        logger.error(`[event] Failed to enrich chunk ${chunkId}:`, { err });
    });
});

// Record usage on chunk view
events.on<{ chunkId: string; userId: string }>(EVENTS.CHUNK_VIEWED, async ({ chunkId, userId }) => {
    Effect.runPromise(recordUsage("chunk_view", [chunkId], userId)).catch(err => {
        logger.error("[event] Failed to record chunk view:", { err });
    });
});

export function registerEventHandlers() {
    // Handlers are registered on import via the `events.on()` calls above.
    // This function exists to be called from the API entry point to ensure
    // the module is loaded and handlers are registered.
    logger.info("[events] Event handlers registered");
}
