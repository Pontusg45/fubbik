import { useMemo } from "react";

interface Heading {
    level: number;
    text: string;
    id: string;
}

function slugify(text: string): string {
    return text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

export function ChunkToc({ content }: { content: string }) {
    const headings = useMemo(() => {
        const lines = content.split("\n");
        const result: Heading[] = [];
        for (const line of lines) {
            const match = line.match(/^(#{2,4})\s+(.+)/);
            if (match) {
                const level = match[1]!.length;
                const text = match[2]!.trim();
                result.push({ level, text, id: slugify(text) });
            }
        }
        return result;
    }, [content]);

    if (headings.length < 2) return null;

    return (
        <nav className="sticky top-8 max-h-[calc(100vh-4rem)] overflow-y-auto print:hidden">
            <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                On this page
            </div>
            <ul className="space-y-1">
                {headings.map((h, i) => (
                    <li key={`${h.id}-${i}`} style={{ paddingLeft: `${(h.level - 2) * 12}px` }}>
                        <a
                            href={`#${h.id}`}
                            className="block truncate text-xs text-muted-foreground hover:text-foreground transition-colors py-1"
                            title={h.text}
                        >
                            {h.text}
                        </a>
                    </li>
                ))}
            </ul>
        </nav>
    );
}
