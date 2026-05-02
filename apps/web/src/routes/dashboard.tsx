import { createFileRoute } from "@tanstack/react-router";

import { PageContainer } from "@/components/ui/page";
import { ErrorBoundary } from "@/components/error-boundary";
import { ActivePlanCard } from "@/features/dashboard/active-plan-card";
import { StatsBar } from "@/features/dashboard/stats-bar";
import { UnifiedFeed } from "@/features/dashboard/unified-feed";
import { getUser } from "@/functions/get-user";

export const Route = createFileRoute("/dashboard")({
    component: DashboardPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    },
});

function DashboardPage() {
    return (
        <PageContainer>
            <div className="mx-auto max-w-3xl space-y-6 py-4">
                <ErrorBoundary fallback={<div className="text-sm text-muted-foreground p-2">Failed to load stats</div>}>
                    <StatsBar />
                </ErrorBoundary>
                <ErrorBoundary fallback={<div className="text-sm text-muted-foreground p-2">Failed to load active plan</div>}>
                    <ActivePlanCard />
                </ErrorBoundary>
                <ErrorBoundary fallback={<div className="text-sm text-muted-foreground p-2">Failed to load feed</div>}>
                    <UnifiedFeed />
                </ErrorBoundary>
            </div>
        </PageContainer>
    );
}
