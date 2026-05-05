import { detectAgeStaleChunks } from "@fubbik/db/repository";
import { env } from "@fubbik/env/server";
import { Effect } from "effect";

import { logger } from "./logger";

const DEV_USER_ID = "dev-user";

async function runStaleScan() {
    const start = Date.now();
    try {
        const result = await Effect.runPromise(detectAgeStaleChunks(DEV_USER_ID));
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
