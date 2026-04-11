import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import { PlanStatusPill, type PlanStatusValue } from "@/features/plans/plan-status-pill";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/")({ component: PlansIndexPage });

function PlansIndexPage() {
    const plansQuery = useQuery({
        queryKey: ["plans"],
        queryFn: async () => {
            const result = unwrapEden(await api.api.plans.get());
            return (result as any[]) ?? [];
        },
    });

    return (
        <PageContainer>
            <PageHeader
                title="Plans"
                description="Plans are the home for a unit of work. Each plan holds its description, linked requirements, analyze notes, and tasks."
                actions={
                    <Button render={<Link to="/plans/new" />}>
                        <Plus className="size-4" />
                        New Plan
                    </Button>
                }
            />
            {plansQuery.isLoading ? (
                <PageLoading />
            ) : (
                <div className="grid gap-2">
                    {(plansQuery.data ?? []).map((p: any) => (
                        <Link
                            key={p.id}
                            to="/plans/$planId"
                            params={{ planId: p.id }}
                            className="flex items-center gap-3 rounded-md border px-4 py-3 transition-colors hover:bg-muted/40"
                        >
                            <span className="flex-1">
                                <div className="font-medium">{p.title}</div>
                                {p.description && (
                                    <div className="text-xs text-muted-foreground line-clamp-1">{p.description}</div>
                                )}
                            </span>
                            <PlanStatusPill status={p.status as PlanStatusValue} />
                            <span className="text-[10px] text-muted-foreground">
                                {new Date(p.updatedAt).toLocaleDateString()}
                            </span>
                        </Link>
                    ))}
                    {(plansQuery.data ?? []).length === 0 && (
                        <div className="rounded-md border border-dashed p-8 text-center text-sm text-muted-foreground">
                            No plans yet.{" "}
                            <Link to="/plans/new" className="underline">
                                Create one
                            </Link>{" "}
                            to get started.
                        </div>
                    )}
                </div>
            )}
        </PageContainer>
    );
}
