import { createFileRoute } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import { RouteErrorBoundary } from "@/components/route-error-boundary";
import { getUser } from "@/functions/get-user";

const GraphView = lazy(() => import("@/features/graph/graph-view"));

export const Route = createFileRoute("/graph")({
    validateSearch: (search: Record<string, unknown>): { pathFrom?: string; pathTo?: string } => ({
        pathFrom: typeof search.pathFrom === "string" ? search.pathFrom : undefined,
        pathTo: typeof search.pathTo === "string" ? search.pathTo : undefined
    }),
    component: () => (
        <RouteErrorBoundary fallbackTitle="Graph failed to render">
            <Suspense
                fallback={
                    <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                        <p className="text-muted-foreground">Loading graph...</p>
                    </div>
                }
            >
                <GraphView />
            </Suspense>
        </RouteErrorBoundary>
    ),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});
