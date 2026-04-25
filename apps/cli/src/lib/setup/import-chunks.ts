import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { DiscoveredChunk, DiscoveredConnection } from "./types";

interface ImportResult {
	chunksCreated: number;
	connectionsCreated: number;
	errors: { item: string; error: string }[];
}

export async function importToServer(
	serverUrl: string,
	codebaseId: string,
	chunks: DiscoveredChunk[],
	connections: DiscoveredConnection[],
	dir: string,
	onProgress?: (message: string) => void,
): Promise<ImportResult> {
	const errors: { item: string; error: string }[] = [];
	let chunksCreated = 0;
	let connectionsCreated = 0;
	const titleToId = new Map<string, string>();

	// Tier 1: Batch import via /api/chunks/import-docs
	const tier1 = chunks.filter(c => c.tier === 1);
	if (tier1.length > 0) {
		onProgress?.("Importing documents...");
		try {
			const sourceFiles = new Map<string, string>();
			for (const chunk of tier1) {
				if (chunk.source && !sourceFiles.has(chunk.source)) {
					const fullPath = join(dir, chunk.source);
					try { sourceFiles.set(chunk.source, readFileSync(fullPath, "utf-8")); } catch {}
				}
			}
			const files = [...sourceFiles.entries()].map(([path, content]) => ({ path, content }));
			if (files.length > 0) {
				const res = await fetch(`${serverUrl}/api/chunks/import-docs`, {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify({ files, codebaseId }),
				});
				if (res.ok) {
					const data = (await res.json()) as { created: number; skipped: number; errors: { path: string; error: string }[] };
					chunksCreated += data.created;
					for (const err of data.errors) errors.push({ item: err.path, error: err.error });
				} else {
					errors.push({ item: "import-docs", error: `Server returned ${res.status}: ${await res.text()}` });
				}
			}
		} catch (err) {
			errors.push({ item: "import-docs", error: String(err) });
		}
	}

	// Tier 2/3: Individual chunk creation
	const tier23 = chunks.filter(c => c.tier !== 1);
	if (tier23.length > 0) {
		onProgress?.("Importing metadata and patterns...");
		for (let i = 0; i < tier23.length; i += 5) {
			const batch = tier23.slice(i, i + 5);
			const results = await Promise.allSettled(
				batch.map(async chunk => {
					const body: Record<string, unknown> = {
						title: chunk.title,
						content: chunk.content,
						type: chunk.type,
						tags: chunk.tags,
						codebaseIds: [codebaseId],
						origin: "ai",
					};
					const res = await fetch(`${serverUrl}/api/chunks`, {
						method: "POST",
						headers: { "Content-Type": "application/json" },
						body: JSON.stringify(body),
					});
					if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
					const data = (await res.json()) as { id: string };
					titleToId.set(chunk.title, data.id);
					return data;
				}),
			);
			for (let j = 0; j < results.length; j++) {
				const result = results[j]!;
				if (result.status === "fulfilled") chunksCreated++;
				else errors.push({ item: batch[j]!.title, error: String(result.reason) });
			}
		}
	}

	// Set appliesTo patterns for chunks that have them
	const chunksWithAppliesTo = tier23.filter(c => c.appliesTo && c.appliesTo.length > 0);
	if (chunksWithAppliesTo.length > 0) {
		onProgress?.("Setting file patterns...");
		for (const chunk of chunksWithAppliesTo) {
			const chunkId = titleToId.get(chunk.title);
			if (!chunkId || !chunk.appliesTo) continue;
			try {
				await fetch(`${serverUrl}/api/chunks/${chunkId}/applies-to`, {
					method: "PUT",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify(chunk.appliesTo.map(pattern => ({ pattern }))),
				});
			} catch {
				// Non-fatal — patterns are a nice-to-have
			}
		}
	}

	// Look up tier-1 chunk IDs for connections
	if (connections.length > 0) {
		onProgress?.("Resolving chunk references...");
		try {
			const res = await fetch(`${serverUrl}/api/chunks?codebaseId=${codebaseId}&limit=200`);
			if (res.ok) {
				const data = (await res.json()) as { items: { id: string; title: string }[] };
				for (const item of data.items) {
					if (!titleToId.has(item.title)) titleToId.set(item.title, item.id);
				}
			}
		} catch {}
	}

	// Connections
	if (connections.length > 0) {
		onProgress?.("Creating connections...");
		for (const conn of connections) {
			const sourceId = titleToId.get(conn.sourceTitle);
			const targetId = titleToId.get(conn.targetTitle);
			if (!sourceId || !targetId) continue;
			try {
				const res = await fetch(`${serverUrl}/api/connections`, {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify({ sourceId, targetId, relation: conn.relation, origin: "ai" }),
				});
				if (res.ok) connectionsCreated++;
			} catch {}
		}
	}

	return { chunksCreated, connectionsCreated, errors };
}
