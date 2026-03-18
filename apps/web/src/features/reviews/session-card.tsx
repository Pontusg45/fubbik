import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";

interface SessionCardProps {
    id: string;
    title: string;
    status: string;
    codebaseName?: string;
    createdAt: string;
}

const statusConfig: Record<string, { label: string; variant: "default" | "secondary" | "outline" }> = {
    in_progress: { label: "In Progress", variant: "default" },
    completed: { label: "Completed", variant: "secondary" },
    reviewed: { label: "Reviewed", variant: "outline" }
};

export function SessionCard({ id, title, status, codebaseName, createdAt }: SessionCardProps) {
    const statusInfo = statusConfig[status] ?? { label: status, variant: "secondary" as const };

    const formattedDate = new Date(createdAt).toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric"
    });

    return (
        <Card>
            <CardPanel className="p-4">
                <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                        <Link
                            to={"/reviews/$sessionId" as string}
                            params={{ sessionId: id } as Record<string, string>}
                            className="text-foreground hover:text-primary font-medium transition-colors"
                        >
                            {title}
                        </Link>
                        <div className="text-muted-foreground mt-1 flex items-center gap-2 text-xs">
                            <span>{formattedDate}</span>
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        {codebaseName && (
                            <Badge variant="outline" className="text-xs">
                                {codebaseName}
                            </Badge>
                        )}
                        <Badge variant={statusInfo.variant}>{statusInfo.label}</Badge>
                    </div>
                </div>
            </CardPanel>
        </Card>
    );
}
