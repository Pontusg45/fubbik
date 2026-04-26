import { useQuery } from "@tanstack/react-query";
import { createContext, useContext, useMemo, type ReactNode } from "react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

/* ─── Types ─── */

export interface ChunkMatch {
    id: string;
    title: string;
}

export interface VocabularyMatch {
    word: string;
    category: string;
    expects: string[] | null;
}

export interface FileRefMatch {
    chunkId: string;
    chunkTitle: string;
    path: string;
}

export type CodeMatch =
    | ({ type: "chunk" } & ChunkMatch)
    | ({ type: "fileRef" } & FileRefMatch)
    | ({ type: "vocabulary" } & VocabularyMatch);

export interface VocabularyTextMatch extends VocabularyMatch {
    start: number;
    end: number;
}

/* ─── Index builders (exported for testing) ─── */

export function buildChunkIndex(
    chunks: { id: string; title: string; aliases: string[] }[]
): Map<string, ChunkMatch> {
    const map = new Map<string, ChunkMatch>();
    for (const c of chunks) {
        if (c.title.length >= 4) {
            map.set(c.title.toLowerCase(), { id: c.id, title: c.title });
        }
        for (const alias of c.aliases) {
            if (alias.length >= 4) {
                map.set(alias.toLowerCase(), { id: c.id, title: c.title });
            }
        }
    }
    return map;
}

export function buildVocabularyIndex(
    entries: { word: string; category: string; expects: string[] | null }[]
): Map<string, VocabularyMatch> {
    const map = new Map<string, VocabularyMatch>();
    for (const e of entries) {
        map.set(e.word.toLowerCase(), { word: e.word, category: e.category, expects: e.expects ?? null });
    }
    return map;
}

export function buildFileRefIndex(
    refs: { chunkId: string; chunkTitle: string; path: string; anchor: string | null }[]
): Map<string, FileRefMatch> {
    const map = new Map<string, FileRefMatch>();
    for (const r of refs) {
        const entry = { chunkId: r.chunkId, chunkTitle: r.chunkTitle, path: r.path };
        map.set(r.path.toLowerCase(), entry);
        // Also index by filename alone for shorter backtick references
        const filename = r.path.split("/").pop();
        if (filename && !map.has(filename.toLowerCase())) {
            map.set(filename.toLowerCase(), entry);
        }
    }
    return map;
}

/* ─── Matching functions (exported for testing) ─── */

/**
 * Match a backticked term. Priority: chunk > fileRef > vocabulary.
 * Optionally exclude a chunk ID (for self-link prevention on detail pages).
 */
export function matchInCode(
    text: string,
    chunkIndex: Map<string, ChunkMatch>,
    fileRefIndex: Map<string, FileRefMatch>,
    vocabIndex: Map<string, VocabularyMatch>,
    excludeChunkId?: string
): CodeMatch | null {
    const lower = text.toLowerCase();

    const chunk = chunkIndex.get(lower);
    if (chunk && chunk.id !== excludeChunkId) {
        return { type: "chunk", ...chunk };
    }

    const fileRef = fileRefIndex.get(lower);
    if (fileRef && fileRef.chunkId !== excludeChunkId) {
        return { type: "fileRef", ...fileRef };
    }

    const vocab = vocabIndex.get(lower);
    if (vocab) {
        return { type: "vocabulary", ...vocab };
    }

    return null;
}

/**
 * Find all vocabulary term matches in a plain text string.
 * Returns matches sorted by position, longest-first for overlapping matches.
 */
export function matchVocabularyInText(
    text: string,
    vocabIndex: Map<string, VocabularyMatch>
): VocabularyTextMatch[] {
    if (vocabIndex.size === 0) return [];

    const words = Array.from(vocabIndex.keys()).sort((a, b) => b.length - a.length);
    const escaped = words.map(w => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
    const pattern = new RegExp(`\\b(${escaped.join("|")})\\b`, "gi");

    const matches: VocabularyTextMatch[] = [];
    let m: RegExpExecArray | null;
    while ((m = pattern.exec(text)) !== null) {
        const vocab = vocabIndex.get(m[1]!.toLowerCase());
        if (vocab) {
            matches.push({ start: m.index, end: m.index + m[0].length, ...vocab });
        }
    }

    return matches;
}

/* ─── Context ─── */

interface SmartLinkContextValue {
    chunkIndex: Map<string, ChunkMatch>;
    vocabIndex: Map<string, VocabularyMatch>;
    fileRefIndex: Map<string, FileRefMatch>;
}

const SmartLinkContext = createContext<SmartLinkContextValue>({
    chunkIndex: new Map(),
    vocabIndex: new Map(),
    fileRefIndex: new Map()
});

export function useSmartLinks() {
    return useContext(SmartLinkContext);
}

/* ─── Provider ─── */

const STALE_TIME = 5 * 60 * 1000; // 5 minutes

export function SmartLinkProvider({ children }: { children: ReactNode }) {
    const chunksQuery = useQuery({
        queryKey: ["smart-link-chunks"],
        queryFn: async () => {
            const result = unwrapEden(await api.api.chunks.get({ query: { limit: "2000" } as any }));
            const chunks = (result as any)?.chunks ?? [];
            return chunks.map((c: any) => ({ id: c.id, title: c.title, aliases: c.aliases ?? [] }));
        },
        staleTime: STALE_TIME
    });

    const vocabQuery = useQuery({
        queryKey: ["smart-link-vocab"],
        queryFn: async () => {
            try {
                // Vocabulary requires a codebaseId — fetch without to get all.
                // If this fails (no codebase selected), return empty.
                const result = unwrapEden(await api.api.vocabulary.get({ query: {} as any }));
                return (result as any[]) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: STALE_TIME
    });

    const fileRefsQuery = useQuery({
        queryKey: ["smart-link-file-refs"],
        queryFn: async () => {
            try {
                const result = unwrapEden(await (api.api as any)["file-refs"].get());
                return (result as any[]) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: STALE_TIME
    });

    const value = useMemo<SmartLinkContextValue>(() => {
        const chunkIndex = buildChunkIndex(chunksQuery.data ?? []);
        const vocabIndex = buildVocabularyIndex(vocabQuery.data ?? []);
        const fileRefIndex = buildFileRefIndex(fileRefsQuery.data ?? []);
        return { chunkIndex, vocabIndex, fileRefIndex };
    }, [chunksQuery.data, vocabQuery.data, fileRefsQuery.data]);

    return (
        <SmartLinkContext.Provider value={value}>
            {children}
        </SmartLinkContext.Provider>
    );
}
