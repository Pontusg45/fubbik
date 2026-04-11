import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { PageContainer, PageLoading } from "@/components/ui/page";
import { PlanDescriptionSection } from "@/features/plans/plan-description-section";
import { PlanDetailHeader } from "@/features/plans/plan-detail-header";
import type { PlanStatusValue } from "@/features/plans/plan-status-pill";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/$planId")({ component: PlanDetailPage });

function PlanDetailPage() {
    const { planId } = Route.useParams();

    const detailQuery = useQuery({
        queryKey: ["plan-detail", planId],
        queryFn: async () => unwrapEden(await (api.api as any).plans[planId].get()),
    });

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
            <div className="space-y-8 pb-12 pt-6">
                <PlanDescriptionSection planId={plan.id} description={plan.description} onUpdate={refetch} />
                {/* Sections added in later tasks:
                    <PlanRequirementsSection /> (Task 14)
                    <PlanAnalyzeSection /> (Task 15)
                    <PlanTasksSection /> (Task 16) */}
            </div>
        </PageContainer>
    );
}
