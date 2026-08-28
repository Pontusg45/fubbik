import { Effect } from "effect";

import { insertGraphEvent } from "../repository/graph-event";
import { createEdge, deleteEdge, deleteVertex, ensureVertex } from "./sync";

function logEvent(
    vertexLabel: string,
    vertexId: string,
    action: string,
    opts?: { edgeType?: string; edgeTargetId?: string; snapshot?: Record<string, unknown> }
) {
    return insertGraphEvent({
        id: crypto.randomUUID(),
        vertexLabel,
        vertexId,
        action,
        ...opts
    }).pipe(Effect.catchAll(() => Effect.succeed(undefined)));
}

export function ensureVertexLogged(label: string, id: string, snapshot?: Record<string, unknown>) {
    return ensureVertex(label, id).pipe(Effect.tap(() => logEvent(label, id, "created", { snapshot })));
}

export function deleteVertexLogged(label: string, id: string) {
    return deleteVertex(label, id).pipe(Effect.tap(() => logEvent(label, id, "deleted")));
}

export function createEdgeLogged(
    edgeLabel: string,
    fromLabel: string,
    fromId: string,
    toLabel: string,
    toId: string,
    props: Record<string, string> = {}
) {
    return createEdge(edgeLabel, fromLabel, fromId, toLabel, toId, props).pipe(
        Effect.tap(() =>
            logEvent(fromLabel, fromId, "created", {
                edgeType: edgeLabel,
                edgeTargetId: toId,
                snapshot: props
            })
        )
    );
}

export function deleteEdgeLogged(edgeLabel: string, props: Record<string, string>) {
    const sourceId = props.id ?? Object.values(props)[0] ?? "unknown";
    return deleteEdge(edgeLabel, props).pipe(
        Effect.tap(() => logEvent("edge", sourceId, "deleted", { edgeType: edgeLabel, snapshot: props }))
    );
}
