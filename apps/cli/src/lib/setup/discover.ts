import type { DiscoveryResult } from "./types";
import { inferConnections } from "./connections";
import { scanDocs } from "./tier1-docs";
import { scanMetadata } from "./tier2-metadata";
import { scanPatterns } from "./tier3-patterns";
import { mergeTips } from "./tips";

export function discover(
	dir: string,
	codebase: { name: string; remoteUrl: string | null; localPath: string },
): DiscoveryResult {
	const tier1Chunks = scanDocs(dir);
	const { chunks: tier2Chunks, tips: tier2Tips } = scanMetadata(dir);
	const { chunks: tier3Chunks, tips: tier3Tips } = scanPatterns(dir);
	const allChunks = [...tier1Chunks, ...tier2Chunks, ...tier3Chunks];
	const connections = inferConnections(allChunks);
	const tips = mergeTips([...tier2Tips, ...tier3Tips]);
	const tags = [...new Set(allChunks.flatMap(c => c.tags))].sort();
	return { codebase, chunks: allChunks, connections, tags, tips };
}
