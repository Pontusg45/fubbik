import { getChunkSize } from "@/features/chunks/chunk-size";

export function SizeIndicator({ length }: { length: number }) {
    const size = getChunkSize(" ".repeat(length));
    const pct = Math.min(100, (length / 5000) * 100);

    return (
        <div className="text-muted-foreground flex items-center gap-2 text-xs">
            <div className="bg-muted h-1.5 w-24 overflow-hidden rounded-full">
                <div className="h-full rounded-full transition-all" style={{ width: `${pct}%`, backgroundColor: size.color }} />
            </div>
            <span>
                {size.label} ({length} chars)
            </span>
        </div>
    );
}
