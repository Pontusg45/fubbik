import Table from "cli-table3";

interface ChunkRow {
    id: string;
    title: string;
    type: string;
    tags?: string[];
    updatedAt?: string;
}

export function formatChunkTable(chunks: ChunkRow[]): string {
    if (chunks.length === 0) return "No chunks found.";

    const table = new Table({
        head: ["ID", "Type", "Title", "Tags", "Updated"],
        colWidths: [12, 12, 40, 20, 12],
        wordWrap: true,
    });

    for (const c of chunks) {
        table.push([
            c.id.slice(0, 10),
            c.type,
            c.title.length > 38 ? c.title.slice(0, 35) + "..." : c.title,
            (c.tags || []).join(", "),
            c.updatedAt ? new Date(c.updatedAt).toLocaleDateString() : "",
        ]);
    }

    return table.toString();
}

interface SemanticRow {
    id: string;
    title: string;
    type: string;
    similarity: number;
}

export function formatSemanticTable(results: SemanticRow[]): string {
    if (results.length === 0) return "No results found.";

    const table = new Table({
        head: ["ID", "Type", "Title", "Score"],
        colWidths: [12, 12, 44, 10],
        wordWrap: true,
    });

    for (const r of results) {
        table.push([
            r.id.slice(0, 10),
            r.type,
            r.title.length > 42 ? r.title.slice(0, 39) + "..." : r.title,
            r.similarity.toFixed(3),
        ]);
    }

    return table.toString();
}
