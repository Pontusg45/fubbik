import pc from "picocolors";
import type { DiscoveredChunk, DiscoveredConnection } from "./types";

const CATEGORY_LABELS: Record<DiscoveredChunk["category"], string> = {
	"documents": "Documents",
	"tech-stack": "Tech stack",
	"structure": "Structure",
	"conventions": "Conventions",
	"config": "Config",
};

const CATEGORY_ORDER: DiscoveredChunk["category"][] = [
	"documents", "tech-stack", "structure", "conventions", "config",
];

export function formatPreview(
	chunks: DiscoveredChunk[],
	connections: DiscoveredConnection[],
	tags: string[],
): string {
	const total = chunks.length;
	const groups = new Map<DiscoveredChunk["category"], DiscoveredChunk[]>();
	for (const chunk of chunks) {
		const list = groups.get(chunk.category) ?? [];
		list.push(chunk);
		groups.set(chunk.category, list);
	}

	const lines: string[] = [];
	lines.push("");
	lines.push(pc.bold(`  Ready to import ${total} chunks`));
	lines.push("");

	const rows: [string, number, string][] = [];
	for (const cat of CATEGORY_ORDER) {
		const catChunks = groups.get(cat);
		if (!catChunks || catChunks.length === 0) continue;
		const examples = catChunks
			.slice(0, 3)
			.map(c => c.title.length > 25 ? c.title.slice(0, 23) + ".." : c.title)
			.join(", ");
		rows.push([CATEGORY_LABELS[cat], catChunks.length, examples]);
	}

	const col1Width = Math.max(...rows.map(r => r[0].length), 10);
	const col2Width = 5;

	for (const [label, count, examples] of rows) {
		const countStr = String(count).padStart(col2Width);
		lines.push(`  ${pc.cyan(label.padEnd(col1Width))} ${countStr}   ${pc.dim(examples)}`);
	}

	lines.push("");

	const footerParts: string[] = [];
	if (connections.length > 0) footerParts.push(`${connections.length} connections`);
	if (tags.length > 0) footerParts.push(`${tags.length} tags`);
	if (footerParts.length > 0) {
		lines.push(`  ${pc.dim("+ " + footerParts.join(", "))}`);
		lines.push("");
	}

	return lines.join("\n");
}

export function formatPreviewJson(
	chunks: DiscoveredChunk[],
	connections: DiscoveredConnection[],
	tags: string[],
): Record<string, unknown> {
	const groups: Record<string, { count: number; titles: string[] }> = {};
	for (const cat of CATEGORY_ORDER) {
		const catChunks = chunks.filter(c => c.category === cat);
		if (catChunks.length > 0) {
			groups[cat] = { count: catChunks.length, titles: catChunks.map(c => c.title) };
		}
	}
	return { totalChunks: chunks.length, groups, connections: connections.length, tags };
}
