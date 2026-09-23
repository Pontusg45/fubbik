import { createFileRoute } from "@tanstack/react-router";

import { PageContainer, PageLoading } from "@/components/ui/page";
import { PlanActivitySidebar } from "@/features/plans/plan-activity-sidebar";
import { PlanAnalyzeSection } from "@/features/plans/plan-analyze-section";
import { usePlanCoordination } from "@/features/plans/plan-coordination-panel";
import { PlanDescriptionSection } from "@/features/plans/plan-description-section";
import { PlanDetailHeader } from "@/features/plans/plan-detail-header";
import { PlanRequirementsSection } from "@/features/plans/plan-requirements-section";
import type { PlanStatusValue } from "@/features/plans/plan-status-pill";
import { PlanTasksSection } from "@/features/plans/plan-tasks-section";
import { usePlanDetail } from "@/features/plans/use-plan-detail";
import { usePlanKeyboardShortcuts } from "@/features/plans/use-plan-keyboard-shortcuts";

export const Route = createFileRoute("/plans/$planId")({ component: PlanDetailPage });

function PlanDetailPage() {
    const { planId } = Route.useParams();
    const detailQuery = usePlanDetail(planId);
    const coordinationQuery = usePlanCoordination(planId);
    usePlanKeyboardShortcuts();

    if (detailQuery.isLoading)
        return (
            <PageContainer>
                <PageLoading />
            </PageContainer>
        );
    if (!detailQuery.data) return <PageContainer>Plan not found</PageContainer>;

    const detail = detailQuery.data;
    const plan = detail.plan;
    const tasks = detail.tasks ?? [];
    const doneCount = tasks.filter(t => t.status === "done").length;

    const refetch = () => {
        void detailQuery.refetch();
    };

    return (
        <PageContainer>
            <PlanDetailHeader
                plan={{ id: plan.id, title: plan.title, status: plan.status as PlanStatusValue, updatedAt: plan.updatedAt }}
                taskCount={{ done: doneCount, total: tasks.length }}
                onUpdate={refetch}
            />
            <div className="flex gap-8 pt-6 pb-12">
                <div className="min-w-0 flex-1 space-y-8">
                    <PlanDescriptionSection planId={plan.id} description={plan.description ?? null} onUpdate={refetch} />
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
                        claims={coordinationQuery.data?.claims}
                        runs={coordinationQuery.data?.runs}
                        onUpdate={refetch}
                    />
                </div>
                <PlanActivitySidebar
                    planId={plan.id}
                    coordination={coordinationQuery.data}
                    coordinationLoading={coordinationQuery.isLoading}
                />
            </div>
        </PageContainer>
    );
}
