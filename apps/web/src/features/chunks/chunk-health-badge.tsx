import { Heart } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

interface HealthScore {
    total: number;
    breakdown: {
        freshness: number;
        completeness: number;
        richness: number;
        connectivity: number;
    };
    issues: string[];
}

function getLabel(score: number): string {
    if (score >= 80) return "Healthy";
    if (score >= 60) return "Fair";
    if (score >= 40) return "Needs attention";
    return "Poor";
}

function getColor(score: number): { text: string; bg: string; border: string } {
    if (score >= 80) return { text: "text-green-600", bg: "bg-green-500/10", border: "border-green-500/30" };
    if (score >= 60) return { text: "text-yellow-600", bg: "bg-yellow-500/10", border: "border-yellow-500/30" };
    if (score >= 40) return { text: "text-orange-600", bg: "bg-orange-500/10", border: "border-orange-500/30" };
    return { text: "text-red-600", bg: "bg-red-500/10", border: "border-red-500/30" };
}

export function ChunkHealthBadge({ healthScore }: { healthScore: HealthScore }) {
    const label = getLabel(healthScore.total);
    const color = getColor(healthScore.total);
    const issueCount = healthScore.issues.length;

    return (
        <TooltipProvider>
            <Tooltip>
                <TooltipTrigger asChild>
                    <Badge variant="outline" className={`${color.bg} ${color.border} ${color.text} gap-1`}>
                        <Heart className="size-3" />
                        {healthScore.total}/100 · {label}
                        {issueCount > 0 && (
                            <span className="ml-0.5 opacity-70">({issueCount})</span>
                        )}
                    </Badge>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="max-w-xs">
                    <div className="space-y-1.5 text-xs">
                        <div className="grid grid-cols-2 gap-x-4 gap-y-0.5">
                            <span>Freshness:</span>
                            <span className="font-medium">{healthScore.breakdown.freshness}/25</span>
                            <span>Completeness:</span>
                            <span className="font-medium">{healthScore.breakdown.completeness}/25</span>
                            <span>Richness:</span>
                            <span className="font-medium">{healthScore.breakdown.richness}/25</span>
                            <span>Connectivity:</span>
                            <span className="font-medium">{healthScore.breakdown.connectivity}/25</span>
                        </div>
                        {issueCount > 0 && (
                            <div className="border-t pt-1.5">
                                <p className="mb-1 font-medium">Issues:</p>
                                <ul className="list-inside list-disc space-y-0.5">
                                    {healthScore.issues.map((issue, i) => (
                                        <li key={i}>{issue}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                    </div>
                </TooltipContent>
            </Tooltip>
        </TooltipProvider>
    );
}
