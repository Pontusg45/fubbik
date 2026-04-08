import { Effect } from "effect";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { cypher, isAgeAvailable } from "./client";
import { createEdge, deleteEdge, deleteVertex, ensureVertex } from "./sync";

const PREFIX = "test-age-sync-";

function uid(name: string) {
    return `${PREFIX}${name}`;
}

let ageReady = false;

beforeAll(async () => {
    ageReady = await isAgeAvailable();
    if (!ageReady) {
        console.log("AGE not available — skipping sync tests");
    }
});

afterAll(async () => {
    if (!ageReady) return;
    // Clean up all test vertices
    const ids = [
        uid("a"),
        uid("b"),
        uid("c"),
        uid("edge-from"),
        uid("edge-to"),
        uid("del-a"),
        uid("del-b"),
        uid("del-center"),
        uid("idempotent")
    ];
    for (const id of ids) {
        await Effect.runPromise(deleteVertex("chunk", id));
    }
});

describe("ensureVertex", () => {
    it("creates a vertex that can be queried back", async () => {
        if (!ageReady) return;
        const id = uid("a");
        await Effect.runPromise(ensureVertex("chunk", id));

        const rows = await Effect.runPromise(
            cypher(
                `MATCH (c:chunk {id: '${id}'}) RETURN c.id AS id`,
                "id agtype"
            )
        );
        expect(rows.length).toBe(1);
        expect((rows[0] as any).id).toContain(id);
    });

    it("is idempotent — calling twice creates only one vertex", async () => {
        if (!ageReady) return;
        const id = uid("idempotent");
        await Effect.runPromise(ensureVertex("chunk", id));
        await Effect.runPromise(ensureVertex("chunk", id));

        const rows = await Effect.runPromise(
            cypher(
                `MATCH (c:chunk {id: '${id}'}) RETURN c.id AS id`,
                "id agtype"
            )
        );
        expect(rows.length).toBe(1);
    });
});

describe("createEdge", () => {
    it("creates an edge between two vertices", async () => {
        if (!ageReady) return;
        const fromId = uid("edge-from");
        const toId = uid("edge-to");

        await Effect.runPromise(ensureVertex("chunk", fromId));
        await Effect.runPromise(ensureVertex("chunk", toId));
        await Effect.runPromise(
            createEdge("connects", "chunk", fromId, "chunk", toId, { relation: "related_to" })
        );

        const rows = await Effect.runPromise(
            cypher(
                `MATCH (a:chunk {id: '${fromId}'})-[e:connects]->(b:chunk {id: '${toId}'}) RETURN e.relation AS rel`,
                "rel agtype"
            )
        );
        expect(rows.length).toBe(1);
    });
});

describe("deleteEdge", () => {
    it("removes an existing edge leaving vertices intact", async () => {
        if (!ageReady) return;
        const fromId = uid("del-a");
        const toId = uid("del-b");

        await Effect.runPromise(ensureVertex("chunk", fromId));
        await Effect.runPromise(ensureVertex("chunk", toId));
        await Effect.runPromise(
            createEdge("connects", "chunk", fromId, "chunk", toId, { relation: "part_of" })
        );

        // Verify edge exists
        const before = await Effect.runPromise(
            cypher(
                `MATCH (a:chunk {id: '${fromId}'})-[e:connects]->(b:chunk {id: '${toId}'}) RETURN 1 AS found`,
                "found agtype"
            )
        );
        expect(before.length).toBe(1);

        // Delete it
        await Effect.runPromise(
            deleteEdge("connects", { relation: "part_of" })
        );

        const after = await Effect.runPromise(
            cypher(
                `MATCH (a:chunk {id: '${fromId}'})-[e:connects {relation: 'part_of'}]->(b:chunk {id: '${toId}'}) RETURN 1 AS found`,
                "found agtype"
            )
        );
        expect(after.length).toBe(0);

        // Vertices still exist
        const verts = await Effect.runPromise(
            cypher(
                `MATCH (c:chunk) WHERE c.id IN ['${fromId}', '${toId}'] RETURN c.id AS id`,
                "id agtype"
            )
        );
        expect(verts.length).toBe(2);
    });
});

describe("deleteVertex", () => {
    it("removes vertex and its edges (DETACH DELETE)", async () => {
        if (!ageReady) return;
        const centerId = uid("del-center");
        const neighborId = uid("b");

        await Effect.runPromise(ensureVertex("chunk", centerId));
        await Effect.runPromise(ensureVertex("chunk", neighborId));
        await Effect.runPromise(
            createEdge("connects", "chunk", centerId, "chunk", neighborId, { relation: "related_to" })
        );

        await Effect.runPromise(deleteVertex("chunk", centerId));

        const rows = await Effect.runPromise(
            cypher(
                `MATCH (c:chunk {id: '${centerId}'}) RETURN c.id AS id`,
                "id agtype"
            )
        );
        expect(rows.length).toBe(0);

        // Edge gone too
        const edges = await Effect.runPromise(
            cypher(
                `MATCH (a:chunk {id: '${centerId}'})-[e]-() RETURN 1 AS found`,
                "found agtype"
            )
        );
        expect(edges.length).toBe(0);
    });
});
