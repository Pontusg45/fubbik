interface PlanProgressBarProps {
    doneCount: number;
    totalSteps: number;
    className?: string;
}

export function PlanProgressBar({ doneCount, totalSteps, className }: PlanProgressBarProps) {
    const percentage = totalSteps > 0 ? Math.round((doneCount / totalSteps) * 100) : 0;

    return (
        <div className={className}>
            <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">
                    {doneCount}/{totalSteps} steps
                </span>
                <span className="font-medium">{percentage}%</span>
            </div>
            <div className="bg-muted mt-1 h-2 overflow-hidden rounded-full">
                <div
                    className="bg-primary h-full rounded-full transition-all duration-300"
                    style={{ width: `${percentage}%` }}
                />
            </div>
        </div>
    );
}
