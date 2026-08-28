import { useCallback, useState } from "react";

interface TimelineScrubberProps {
    earliest: Date;
    latest: Date;
    onTimeChange: (time: Date | null) => void;
    isPlaying: boolean;
    onPlayToggle: () => void;
}

export function TimelineScrubber({ earliest, latest, onTimeChange, isPlaying, onPlayToggle }: TimelineScrubberProps) {
    const range = latest.getTime() - earliest.getTime();
    const [value, setValue] = useState(1);

    const handleChange = useCallback(
        (e: React.ChangeEvent<HTMLInputElement>) => {
            const v = Number(e.target.value);
            setValue(v);
            if (v >= 1) {
                onTimeChange(null);
            } else {
                const time = new Date(earliest.getTime() + v * range);
                onTimeChange(time);
            }
        },
        [earliest, range, onTimeChange]
    );

    const displayDate = value >= 1 ? "Now" : new Date(earliest.getTime() + value * range).toLocaleDateString();

    return (
        <div className="bg-background/80 flex items-center gap-2 rounded-lg border px-3 py-1.5 backdrop-blur">
            <button onClick={onPlayToggle} className="hover:text-primary text-sm" aria-label={isPlaying ? "Pause" : "Play"}>
                {isPlaying ? "||" : ">"}
            </button>
            <input type="range" min={0} max={1} step={0.001} value={value} onChange={handleChange} className="accent-primary w-48" />
            <span className="text-muted-foreground min-w-[5rem] text-xs">{displayDate}</span>
        </div>
    );
}

export default TimelineScrubber;
