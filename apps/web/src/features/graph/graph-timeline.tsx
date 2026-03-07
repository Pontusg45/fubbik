import { Calendar, Pause, Play } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

interface GraphTimelineProps {
    chunks: { id: string; createdAt: string | Date }[];
    onCutoffChange: (cutoff: Date | null) => void;
}

export function GraphTimeline({ chunks, onCutoffChange }: GraphTimelineProps) {
    const [expanded, setExpanded] = useState(false);
    const [playing, setPlaying] = useState(false);
    const [value, setValue] = useState(100);
    const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const range = useMemo(() => {
        if (chunks.length === 0) return null;
        const dates = chunks.map(c => new Date(c.createdAt).getTime()).sort((a, b) => a - b);
        return { min: dates[0]!, max: dates[dates.length - 1]! };
    }, [chunks]);

    const cutoff = useMemo(() => {
        if (!range || value >= 100) return null;
        return new Date(range.min + ((range.max - range.min) * value) / 100);
    }, [range, value]);

    useEffect(() => { onCutoffChange(cutoff); }, [cutoff, onCutoffChange]);

    useEffect(() => {
        if (!playing) { if (intervalRef.current) clearInterval(intervalRef.current); return; }
        intervalRef.current = setInterval(() => {
            setValue(prev => {
                if (prev >= 100) { setPlaying(false); return 100; }
                return prev + 1;
            });
        }, 100);
        return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
    }, [playing]);

    if (!range) return null;

    const visibleCount = cutoff
        ? chunks.filter(c => new Date(c.createdAt) <= cutoff).length
        : chunks.length;

    return (
        <div className="absolute bottom-12 left-1/2 z-10 -translate-x-1/2 rounded-lg border bg-background/90 backdrop-blur-sm">
            <button
                onClick={() => setExpanded(!expanded)}
                className="flex items-center gap-2 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground"
            >
                <Calendar className="size-3" />
                Timeline
            </button>
            {expanded && (
                <div className="border-t px-3 py-2 flex items-center gap-3">
                    <button
                        onClick={() => { if (value >= 100) setValue(0); setPlaying(!playing); }}
                        className="text-muted-foreground hover:text-foreground"
                    >
                        {playing ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
                    </button>
                    <input
                        type="range"
                        min={0}
                        max={100}
                        value={value}
                        onChange={e => { setValue(Number(e.target.value)); setPlaying(false); }}
                        className="w-48"
                    />
                    <span className="text-[10px] text-muted-foreground whitespace-nowrap">
                        {cutoff ? cutoff.toLocaleDateString() : "All"} ({visibleCount})
                    </span>
                </div>
            )}
        </div>
    );
}
