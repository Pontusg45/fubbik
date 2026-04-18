import { useQuery } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";

import { PageContainer, PageLoading } from "@/components/ui/page";
import { PlanActivitySidebar } from "@/features/plans/plan-activity-sidebar";
import { PlanAnalyzeSection } from "@/features/plans/plan-analyze-section";
import { PlanDescriptionSection } from "@/features/plans/plan-description-section";
import { PlanDetailHeader } from "@/features/plans/plan-detail-header";
import { PlanRequirementsSection } from "@/features/plans/plan-requirements-section";
import type { PlanStatusValue } from "@/features/plans/plan-status-pill";
import { PlanTasksSection } from "@/features/plans/plan-tasks-section";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/$planId")({ component: PlanDetailPage });

function PlanDetailPage() {
    const { planId } = Route.useParams();
    const navigate = useNavigate();

    const detailQuery = useQuery({
        queryKey: ["plan-detail", planId],
        queryFn: async () => unwrapEden(await (api.api as any).plans[planId].get()),
    });

    // Keyboard shortcuts for the detail page:
    //   `a` opens the add-task input (focuses the page-level "Add task" button)
    //   `Cmd/Ctrl+D` triggers the duplicate icon in the header
    //   `Esc` returns to /plans
    useEffect(() => {
        function onKey(e: KeyboardEvent) {
            const target = e.target as HTMLElement | null;
            if (target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || target?.isContentEditable) return;

            if (e.key === "a" && !e.metaKey && !e.ctrlKey) {
                const btn = document.querySelector<HTMLButtonElement>('button[data-plan-add-task]');
                btn?.click();
            } else if ((e.key === "d" || e.key === "D") && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                const btn = document.querySelector<HTMLButtonElement>('button[title="Duplicate"]');
                btn?.click();
            } else if (e.key === "Escape") {
                navigate({ to: "/plans" });
            }
        }
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [navigate]);

    if (detailQuery.isLoading) return <PageContainer><PageLoading /></PageContainer>;
    if (!detailQuery.data) return <PageContainer>Plan not found</PageContainer>;

    const detail = detailQuery.data as any;
    const plan = detail.plan;
    const tasks = detail.tasks ?? [];
    const doneCount = tasks.filter((t: any) => t.status === "done").length;

    const refetch = () => { void detailQuery.refetch(); };

    return (
        <PageContainer>
            <PlanDetailHeader
                plan={{ id: plan.id, title: plan.title, status: plan.status as PlanStatusValue, updatedAt: plan.updatedAt }}
                taskCount={{ done: doneCount, total: tasks.length }}
                onUpdate={refetch}
            />
            <div className="flex gap-8 pb-12 pt-6">
                <div className="min-w-0 flex-1 space-y-8">
                    <PlanDescriptionSection planId={plan.id} description={plan.description} onUpdate={refetch} />
                    <PlanRequirementsSection planId={plan.id} requirements={detail.requirements ?? []} onUpdate={refetch} />
                    <PlanAnalyzeSection
                        planId={plan.id}
                        analyze={detail.analyze ?? { chunk: [], file: [], risk: [], assumption: [], question: [] }}
                        onUpdate={refetch}
                    />
                    <PlanTasksSection
                        planId={plan.id}
                        tasks={tasks}
                        dependencies={detail.dependencies ?? []}
                        onUpdate={refetch}
                    />
                </div>
                <PlanActivitySidebar planId={plan.id} />
            </div>
        </PageContainer>
    );
}
