interface QueryInputProps {
    value: string;
    onChange: (value: string) => void;
    onSubmit: (value: string) => void;
}

export function QueryInput({ value, onChange, onSubmit }: QueryInputProps) {
    function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === "Enter") {
            onSubmit(value);
        }
    }

    return (
        <div className="space-y-1.5">
            <input
                type="text"
                value={value}
                onChange={e => onChange(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="type:note tag:architecture near:chunk-id:2 NOT text:deprecated"
                className="w-full rounded-md border bg-background px-3 py-2 font-mono text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-ring"
                spellCheck={false}
                autoComplete="off"
            />
            <p className="text-[10px] text-muted-foreground/60">
                Supports:{" "}
                <span className="font-mono">
                    type: tag: connections: updated: origin: review: near: hops: path: affected-by: NOT
                </span>
                {" "}· Press <span className="font-mono">Enter</span> to search
            </p>
        </div>
    );
}
