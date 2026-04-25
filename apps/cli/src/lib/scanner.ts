import { scanDocs } from "./setup/tier1-docs";

export interface ScannedChunk {
    title: string;
    content: string;
    type: string;
    tags: string[];
    /** Directory relative to project root, used to group related chunks */
    folder: string;
    /** Whether this chunk is the index/README for its folder */
    isIndex?: boolean;
    /** If this chunk was split from a larger file, the title of the parent index chunk */
    parentTitle?: string;
}

interface ScanOptions {
    dir: string;
    verbose?: boolean;
}

export function scanProject(opts: ScanOptions): ScannedChunk[] {
    const discovered = scanDocs(opts.dir);
    return discovered.map(d => ({
        title: d.title,
        content: d.content,
        type: d.type,
        tags: d.tags,
        folder: ".",
        isIndex: false,
    }));
}
