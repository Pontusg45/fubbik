import { getGraphEventsBetween, getGraphEventsUpTo } from "@fubbik/db/repository";
import { Effect } from "effect";

export interface TimelineNode {
    id: string;
    label: string;
    properties: Record<string, unknown>;
}

export interface TimelineEdge {
    sourceId: string;
    targetId: string;
    type: string;
    properties: Record<string, unknown>;
}

export interface TimelineGraph {
    nodes: TimelineNode[];
    edges: TimelineEdge[];
    eventCount: number;
}

export function reconstructGraphAt(timestamp: Date) {
    return Effect.gen(function* () {
        const events = yield* getGraphEventsUpTo(timestamp);

        const nodes = new Map<string, TimelineNode>();
        const edges = new Map<string, TimelineEdge>();

        for (const event of events) {
            const nodeKey = `${event.vertexLabel}:${event.vertexId}`;

            if (!event.edgeType) {
                if (event.action === "created" || event.action === "updated" || event.action === "property_changed") {
                    const existing = nodes.get(nodeKey);
                    nodes.set(nodeKey, {
                        id: event.vertexId,
                        label: event.vertexLabel,
                        properties: { ...(existing?.properties ?? {}), ...((event.snapshot as Record<string, unknown>) ?? {}) }
                    });
                } else if (event.action === "deleted") {
                    nodes.delete(nodeKey);
                }
            } else {
                const edgeKey = `${event.vertexId}-${event.edgeType}-${event.edgeTargetId}`;
                if (event.action === "created") {
                    edges.set(edgeKey, {
                        sourceId: event.vertexId,
                        targetId: event.edgeTargetId!,
                        type: event.edgeType,
                        properties: (event.snapshot as Record<string, unknown>) ?? {}
                    });
                } else if (event.action === "deleted") {
                    edges.delete(edgeKey);
                }
            }
        }

        return {
            nodes: Array.from(nodes.values()),
            edges: Array.from(edges.values()),
            eventCount: events.length
        } satisfies TimelineGraph;
    });
}

export { getGraphEventsBetween };
