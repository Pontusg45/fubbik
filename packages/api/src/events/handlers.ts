import { Effect } from "effect";

import { enrichChunkIfEmpty } from "../enrich/service";
import { events, EVENTS } from "./bus";

// Auto-enrich on chunk create
events.on<{ chunkId: string; userId: string }>(EVENTS.CHUNK_CREATED, async ({ chunkId }) => {
    Effect.runPromise(enrichChunkIfEmpty(chunkId)).catch(err => {
        console.error(`[event] Failed to enrich chunk ${chunkId}:`, err);
    });
});

export function registerEventHandlers() {
    // Handlers are registered on import via the `events.on()` calls above.
    // This function exists to be called from the API entry point to ensure
    // the module is loaded and handlers are registered.
    console.log("[events] Event handlers registered");
}
