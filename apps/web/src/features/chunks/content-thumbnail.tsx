import { useMemo } from "react";

export function ContentThumbnail({ content, className }: { content: string | null | undefined; className?: string }) {
    const lines = useMemo(() => {
        if (!content) return [];
        return content
            .split("\n")
            .slice(0, 20)
            .map(line => Math.min(100, line.trim().length));
    }, [content]);

    if (lines.length === 0) return null;

    return (
        <svg
            className={className ?? "h-16 w-full"}
            viewBox="0 0 100 80"
            preserveAspectRatio="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            {lines.map((width, i) => (
                <rect
                    key={i}
                    x={2}
                    y={i * 4}
                    width={width}
                    height={1.5}
                    fill="currentColor"
                    opacity={0.2}
                    rx={0.5}
                />
            ))}
        </svg>
    );
}
