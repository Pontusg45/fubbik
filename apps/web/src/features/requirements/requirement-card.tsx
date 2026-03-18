import { Link } from "@tanstack/react-router";
import { Bot, GripVertical } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";

interface RequirementCardProps {
    id: string;
    title: string;
    status: string;
    priority: string | null;
    steps: Array<{ keyword: string; text: string }>;
    origin: string;
    reviewStatus: string;
    useCaseName?: string;
    chunkCount?: number;
    selected: boolean;
    onSelectChange: (selected: boolean) => void;
    dragHandleProps?: React.HTMLAttributes<HTMLDivElement>;
}

function statusColor(status: string) {
    switch (status) {
        case "passing":
            return "text-green-600 bg-green-500/10 border-green-500/30";
        case "failing":
            return "text-red-600 bg-red-500/10 border-red-500/30";
        default:
            return "text-muted-foreground bg-muted";
    }
}

function priorityLabel(priority: string) {
    switch (priority) {
        case "must":
            return "Must";
        case "should":
            return "Should";
        case "could":
            return "Could";
        case "wont":
            return "Won't";
        default:
            return priority;
    }
}

function reviewStatusColor(reviewStatus: string) {
    switch (reviewStatus) {
        case "draft":
            return "border-yellow-500/30 bg-yellow-500/10 text-[10px] text-yellow-600";
        case "reviewed":
            return "border-blue-500/30 bg-blue-500/10 text-[10px] text-blue-600";
        default:
            return "border-green-500/30 bg-green-500/10 text-[10px] text-green-600";
    }
}

function buildStepPreview(steps: Array<{ keyword: string; text: string }>): string {
    const given = steps.find(s => s.keyword === "given");
    const when = steps.find(s => s.keyword === "when");
    const then = steps.find(s => s.keyword === "then");

    const parts: string[] = [];
    if (given) parts.push(`Given ${given.text}`);
    if (when) parts.push(`When ${when.text}`);
    if (then) parts.push(`Then ${then.text}`);

    const preview = parts.join(" \u00b7 ");
    return preview.length > 120 ? preview.slice(0, 117) + "..." : preview;
}

export function RequirementCard({
    id,
    title,
    status,
    priority,
    steps,
    origin,
    reviewStatus,
    useCaseName,
    chunkCount,
    selected,
    onSelectChange,
    dragHandleProps
}: RequirementCardProps) {
    const isFailing = status === "failing";

    return (
        <Card
            className={
                isFailing ? "border-red-500/30 bg-red-500/5" : undefined
            }
        >
            <CardPanel className="p-4">
                <div className="flex items-start gap-3">
                    {dragHandleProps && (
                        <div {...dragHandleProps} className="cursor-grab text-muted-foreground hover:text-foreground touch-none">
                            <GripVertical className="size-4" />
                        </div>
                    )}
                    <div className="pt-0.5">
                        <Checkbox
                            checked={selected}
                            onCheckedChange={checked => onSelectChange(checked === true)}
                        />
                    </div>
                    <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                            <Link
                                to="/requirements/$requirementId"
                                params={{ requirementId: id }}
                                className="font-medium hover:underline"
                            >
                                {title}
                            </Link>
                            <Badge variant="outline" size="sm" className={statusColor(status)}>
                                {status}
                            </Badge>
                            {priority && (
                                <Badge variant="secondary" size="sm">
                                    {priorityLabel(priority)}
                                </Badge>
                            )}
                            {origin === "ai" && (
                                <Badge variant="outline" size="sm" className={reviewStatusColor(reviewStatus)}>
                                    <Bot className="mr-0.5 size-2.5" />
                                    AI
                                </Badge>
                            )}
                            {useCaseName && (
                                <Badge variant="outline" size="sm" className="border-indigo-500/30 bg-indigo-500/10 text-[10px] text-indigo-600">
                                    {useCaseName}
                                </Badge>
                            )}
                        </div>
                        {steps.length > 0 && (
                            <p className="text-muted-foreground mt-1 text-xs leading-relaxed">
                                {buildStepPreview(steps)}
                            </p>
                        )}
                        <div className="text-muted-foreground mt-2 flex items-center gap-3 text-xs">
                            <span>{steps.length} step{steps.length !== 1 ? "s" : ""}</span>
                            {chunkCount !== undefined && chunkCount > 0 && (
                                <span>{chunkCount} chunk{chunkCount !== 1 ? "s" : ""}</span>
                            )}
                        </div>
                    </div>
                </div>
            </CardPanel>
        </Card>
    );
}
