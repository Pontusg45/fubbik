import { fetchTimeline } from "@fubbik/db/repository";
import { Effect } from "effect";

function parseRange(range: string): number {
    const match = /^(\d+)([dwmy])$/.exec(range);
    if (!match) return 30;
    const n = Number(match[1]);
    switch (match[2]) {
        case "d":
            return n;
        case "w":
            return n * 7;
        case "m":
            return n * 30;
        case "y":
            return n * 365;
        default:
            return 30;
    }
}

export interface TimelineOpts {
    range?: string;
    codebaseId?: string;
    tag?: string;
}

export function getTimeline(userId: string, opts: TimelineOpts) {
    const days = parseRange(opts.range ?? "30d");
    const from = new Date(Date.now() - days * 86400_000);

    return fetchTimeline({
        userId,
        from,
        codebaseId: opts.codebaseId,
        tag: opts.tag
    }).pipe(
        Effect.map(events => {
            const totals = events.reduce(
                (acc, e) => {
                    if (e.kind === "created") acc.created++;
                    else acc.updated++;
                    return acc;
                },
                { created: 0, updated: 0 }
            );
            return {
                events,
                totals,
                range: { from: from.toISOString(), to: new Date().toISOString(), days }
            };
        })
    );
}
