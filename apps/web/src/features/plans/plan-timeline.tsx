interface PlanTimelineProps {
    steps: Array<{
        id: string;
        description: string;
        status: string;
        order: number;
    }>;
}

const statusColor: Record<string, string> = {
    done: "bg-green-500",
    in_progress: "bg-blue-500",
    blocked: "bg-red-500",
    skipped: "bg-muted-foreground/30",
    pending: "bg-muted-foreground/20",
};

const statusTextColor: Record<string, string> = {
    done: "text-green-900 dark:text-green-100",
    in_progress: "text-blue-900 dark:text-blue-100",
    blocked: "text-red-900 dark:text-red-100",
    skipped: "",
    pending: "",
};

export function PlanTimeline({ steps }: PlanTimelineProps) {
    if (steps.length === 0) return null;
    const maxOrder = Math.max(...steps.map(s => s.order), 1);

    return (
        <div className="space-y-1.5">
            {steps.map(step => (
                <div key={step.id} className="flex items-center gap-2">
                    <span className="text-muted-foreground w-6 text-right text-[10px] font-mono shrink-0">
                        {step.order + 1}
                    </span>
                    <div className="flex-1 relative h-6 rounded bg-muted/30 overflow-hidden">
                        <div
                            className={`absolute inset-y-0 left-0 rounded ${statusColor[step.status] ?? "bg-muted"}`}
                            style={{ width: `${((step.order + 1) / (maxOrder + 1)) * 100}%` }}
                        />
                        <span className={`relative z-10 px-2 text-[11px] leading-6 truncate block ${statusTextColor[step.status] ?? ""}`}>
                            {step.description}
                        </span>
                    </div>
                </div>
            ))}
        </div>
    );
}
