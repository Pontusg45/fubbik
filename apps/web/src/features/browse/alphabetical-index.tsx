import { Link } from "@tanstack/react-router";
import { useMemo } from "react";

interface ChunkRef {
    id: string;
    title: string;
    type: string;
}

const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

export function AlphabeticalIndex({ chunks }: { chunks: ChunkRef[] }) {
    const groups = useMemo(() => {
        const map = new Map<string, ChunkRef[]>();
        for (const c of chunks) {
            const letter = (c.title.trim()[0] || "#").toUpperCase();
            const key = /[A-Z]/.test(letter) ? letter : "#";
            const existing = map.get(key) ?? [];
            existing.push(c);
            map.set(key, existing);
        }
        for (const letter of LETTERS) {
            if (!map.has(letter)) map.set(letter, []);
        }
        if (!map.has("#")) map.set("#", []);
        const sorted = Array.from(map.entries()).sort(([a], [b]) => {
            if (a === "#") return 1;
            if (b === "#") return -1;
            return a.localeCompare(b);
        });
        for (const [, items] of sorted) {
            items.sort((a, b) => a.title.localeCompare(b.title));
        }
        return sorted;
    }, [chunks]);

    return (
        <div className="space-y-8">
            {/* Jump bar */}
            <div className="sticky top-0 z-10 flex flex-wrap gap-1 bg-background/95 backdrop-blur py-3 border-b">
                {groups.map(([letter, items]) => (
                    <a
                        key={letter}
                        href={`#letter-${letter}`}
                        className={`rounded px-2 py-1 text-xs font-mono transition-colors ${
                            items.length > 0
                                ? "hover:bg-muted text-foreground"
                                : "text-muted-foreground/30 pointer-events-none"
                        }`}
                    >
                        {letter}
                    </a>
                ))}
            </div>

            {/* Groups */}
            {groups.map(([letter, items]) => (
                items.length > 0 && (
                    <section key={letter} id={`letter-${letter}`}>
                        <h2 className="mb-3 border-b pb-1 text-lg font-bold">{letter}</h2>
                        <div className="grid gap-1 sm:grid-cols-2 lg:grid-cols-3">
                            {items.map(item => (
                                <Link
                                    key={item.id}
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: item.id }}
                                    className="hover:bg-muted/50 rounded px-2 py-1 text-sm transition-colors"
                                >
                                    {item.title}
                                    <span className="ml-2 text-[9px] text-muted-foreground font-mono">
                                        {item.type}
                                    </span>
                                </Link>
                            ))}
                        </div>
                    </section>
                )
            ))}
        </div>
    );
}
