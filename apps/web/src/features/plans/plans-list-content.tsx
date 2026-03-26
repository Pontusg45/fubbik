import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ClipboardList } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { Empty, EmptyAction, EmptyDescription, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { Button } from "@/components/ui/button";
import { PlanProgressBar } from "@/features/plans/plan-progress-bar";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface Plan {
    id: string;
    title: string;
    description: string | null;
    status: string;
    totalSteps: number;
    doneCount: number;
    createdAt: Date;
    updatedAt: Date;
}

const statusVariant: Record<string, "default" | "secondary" | "outline" | "destructive"> = {
    draft: "secondary",
    active: "default",
    completed: "outline",
    archived: "outline"
};

export function PlansListContent() {
    const plansQuery = useQuery({
        queryKey: ["plans"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.plans.get({ query: {} })) as Plan[];
            } catch {
                return [];
            }
        }
    });

    const plans = Array.isArray(plansQuery.data) ? plansQuery.data : [];

    return (
        <Card>
            <CardPanel className="p-6">
                {plansQuery.isLoading ? (
                    <SkeletonList count={4} />
                ) : plans.length === 0 ? (
                    <Empty>
                        <EmptyMedia variant="icon"><ClipboardList className="h-10 w-10" /></EmptyMedia>
                        <EmptyTitle>No plans yet</EmptyTitle>
                        <EmptyDescription>Plans help you track multi-step tasks and projects.</EmptyDescription>
                        <EmptyAction>
                            <Button render={<Link to="/plans/new" />}>Create Plan</Button>
                        </EmptyAction>
                    </Empty>
                ) : (
                    <div className="divide-y">
                        {plans.map(p => (
                            <Link
                                key={p.id}
                                to="/plans/$planId"
                                params={{ planId: p.id }}
                                className="hover:bg-muted/50 block py-3 px-1 first:pt-0 last:pb-0 transition-colors rounded-sm"
                            >
                                <div className="flex items-start justify-between gap-4">
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{p.title}</span>
                                            <Badge variant={statusVariant[p.status] ?? "secondary"} size="sm">
                                                {p.status}
                                            </Badge>
                                        </div>
                                        {p.description && (
                                            <p className="text-muted-foreground mt-0.5 text-sm line-clamp-1">
                                                {p.description}
                                            </p>
                                        )}
                                    </div>
                                </div>
                                {p.totalSteps > 0 && (
                                    <PlanProgressBar
                                        doneCount={p.doneCount}
                                        totalSteps={p.totalSteps}
                                        className="mt-2"
                                    />
                                )}
                            </Link>
                        ))}
                    </div>
                )}
            </CardPanel>
        </Card>
    );
}
