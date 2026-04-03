import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, X } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface SessionCardProps {
    id: string;
    title: string;
    status: string;
    codebaseName?: string;
    createdAt: string;
}

const statusConfig: Record<string, { label: string; variant: "default" | "secondary" | "outline"; className?: string }> = {
    in_progress: { label: "In Progress", variant: "secondary", className: "bg-yellow-500/10 text-yellow-600 border-yellow-500/30 dark:text-yellow-400" },
    completed: { label: "Completed", variant: "secondary", className: "bg-blue-500/10 text-blue-600 border-blue-500/30 dark:text-blue-400" },
    reviewed: { label: "Reviewed", variant: "secondary", className: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400" }
};

export function SessionCard({ id, title, status, codebaseName, createdAt }: SessionCardProps) {
    const queryClient = useQueryClient();
    const statusInfo = statusConfig[status] ?? { label: status, variant: "secondary" as const };

    const approveMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(
                await (api.api.sessions as any)({ id }).review.patch({})
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["sessions"] });
            toast.success("Session approved");
        },
        onError: () => toast.error("Failed to approve session")
    });

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
                        <Badge variant={statusInfo.variant} className={statusInfo.className}>
                            {statusInfo.label}
                        </Badge>
                        {status === "completed" && (
                            <div className="flex gap-1">
                                <button
                                    onClick={(e) => { e.preventDefault(); approveMutation.mutate(); }}
                                    className="text-green-600 hover:bg-green-50 dark:hover:bg-green-950 rounded-md p-1.5 transition-colors"
                                    title="Approve"
                                    disabled={approveMutation.isPending}
                                >
                                    <Check className="size-4" />
                                </button>
                                <button
                                    onClick={(e) => { e.preventDefault(); }}
                                    className="text-red-600 hover:bg-red-50 dark:hover:bg-red-950 rounded-md p-1.5 transition-colors"
                                    title="Reject"
                                >
                                    <X className="size-4" />
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            </CardPanel>
        </Card>
    );
}
