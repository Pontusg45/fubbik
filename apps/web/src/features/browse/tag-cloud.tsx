import { Link } from "@tanstack/react-router";
import { useMemo } from "react";

interface TagWithCount {
    name: string;
    count: number;
}

export function TagCloud({ tags }: { tags: TagWithCount[] }) {
    const normalized = useMemo(() => {
        if (tags.length === 0) return [];
        const counts = tags.map(t => t.count);
        const min = Math.min(...counts);
        const max = Math.max(...counts);
        const range = max - min || 1;
        return tags.map(t => ({
            ...t,
            weight: (t.count - min) / range,
        }));
    }, [tags]);

    function fontSize(weight: number): string {
        // 0.75rem (xs) to 1.5rem (2xl)
        const size = 0.75 + weight * 0.75;
        return `${size}rem`;
    }

    function opacity(weight: number): number {
        return 0.5 + weight * 0.5;
    }

    if (tags.length === 0) {
        return (
            <div className="py-16 text-center text-sm text-muted-foreground">
                No tags found.
            </div>
        );
    }

    return (
        <div className="flex flex-wrap items-center justify-center gap-x-3 gap-y-2 p-6">
            {normalized.map(tag => (
                <Link
                    key={tag.name}
                    to="/chunks"
                    search={{ tags: tag.name } as any}
                    className="hover:text-primary transition-colors"
                    style={{ fontSize: fontSize(tag.weight), opacity: opacity(tag.weight) }}
                    title={`${tag.count} chunk${tag.count === 1 ? "" : "s"}`}
                >
                    {tag.name}
                </Link>
            ))}
        </div>
    );
}
