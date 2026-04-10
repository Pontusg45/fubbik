import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { MarkdownRenderer } from "@/components/markdown-renderer";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

/**
 * Renders chunk content as markdown with auto-linked chunk titles.
 * Matches titles (case-insensitive) and wraps them in markdown links.
 */
export function ChunkLinkRenderer({ content, currentChunkId }: { content: string; currentChunkId?: string }) {
    const { data: allChunks } = useQuery({
        queryKey: ["chunks-title-index"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
        staleTime: 300_000, // 5 min cache
    });

    const titleMap = useMemo(() => {
        if (!allChunks) return new Map<string, string>();
        const map = new Map<string, string>();
        const chunks = (allChunks as any)?.chunks ?? [];
        for (const c of chunks) {
            // Skip the current chunk, and skip very short titles (too noisy)
            if (c.id !== currentChunkId && c.title && c.title.length >= 4) {
                map.set(c.title.toLowerCase(), c.id);
            }
        }
        return map;
    }, [allChunks, currentChunkId]);

    const processed = useMemo(() => {
        if (titleMap.size === 0) return content;
        // Build a regex matching all titles, sorted longest first so longer titles win
        const titles = Array.from(titleMap.keys()).sort((a, b) => b.length - a.length);
        if (titles.length === 0) return content;
        const escaped = titles.map(t => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
        const pattern = new RegExp(`\\b(${escaped.join("|")})\\b`, "gi");
        // Replace while avoiding already-linked text (inside [text](url))
        // Simple approach: skip replacement inside markdown links
        let result = "";
        let lastIdx = 0;
        const linkPattern = /\[[^\]]*\]\([^)]*\)/g;
        const linkRanges: Array<[number, number]> = [];
        let lm;
        while ((lm = linkPattern.exec(content)) !== null) {
            linkRanges.push([lm.index, lm.index + lm[0].length]);
        }
        const isInsideLink = (idx: number) => linkRanges.some(([s, e]) => idx >= s && idx < e);

        let m;
        while ((m = pattern.exec(content)) !== null) {
            if (isInsideLink(m.index)) continue;
            const id = titleMap.get(m[0].toLowerCase());
            if (!id) continue;
            result += content.slice(lastIdx, m.index);
            result += `[${m[0]}](/chunks/${id})`;
            lastIdx = m.index + m[0].length;
        }
        result += content.slice(lastIdx);
        return result;
    }, [content, titleMap]);

    return <MarkdownRenderer>{processed}</MarkdownRenderer>;
}
