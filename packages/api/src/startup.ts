import { IMPLICIT_DEV_USER_ID, detectAgeStaleChunks, ensureImplicitDevUserRow } from "@fubbik/db/repository";
import { env } from "@fubbik/env/server";
import { Effect } from "effect";

import { logger } from "./logger";

export function apiUsesImplicitDevUser(): boolean {
    return env.NODE_ENV !== "production" || env.FUBBIK_IMPLICIT_DEV_SESSION === "true";
}

/** Await once before accepting traffic so FKs to `dev-user` never race an empty DB. */
export async function awaitImplicitDevUserBootstrap(): Promise<void> {
    if (!apiUsesImplicitDevUser()) return;
    try {
        const inserted = await ensureImplicitDevUserRow();
        if (inserted) logger.info("Created implicit dev user row (dev-user)");
    } catch (err) {
        logger.error("Failed to ensure implicit dev user row", { error: err });
        throw err;
    }
}

async function runStaleScan() {
    const start = Date.now();
    try {
        const result = await Effect.runPromise(detectAgeStaleChunks(IMPLICIT_DEV_USER_ID));
        const duration = Date.now() - start;
        logger.info("Staleness scan completed", {
            flagged: result.flagged,
            durationMs: duration,
        });
    } catch (err) {
        logger.error("Staleness scan failed", { error: err });
    }
}

export function initStartupTasks() {
    const intervalHours = Number(env.STALENESS_SCAN_INTERVAL_HOURS ?? "24");
    if (intervalHours <= 0) {
        logger.info("Staleness scanning disabled (STALENESS_SCAN_INTERVAL_HOURS=0)");
        return;
    }

    // Run initial scan after a delay (give migrations time to complete in Docker)
    setTimeout(() => {
        runStaleScan();
    }, 30000);

    // Schedule recurring scans
    const intervalMs = intervalHours * 60 * 60 * 1000;
    setInterval(() => {
        runStaleScan();
    }, intervalMs);

    logger.info(`Staleness scanning enabled (every ${intervalHours}h)`);
}
