import { createFileRoute } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import { RouteErrorBoundary } from "@/components/route-error-boundary";
import { getUser } from "@/functions/get-user";

const GraphView = lazy(() => import("@/features/graph/graph-view"));

export interface GraphSearch {
    pathFrom?: string;
    pathTo?: string;
    focus?: string;
    tagTypeId?: string;
    zoomLevel?: "overview" | "neighborhood" | "detail";
    island?: string;
}

export const Route = createFileRoute("/graph")({
    validateSearch: (search: Record<string, unknown>): GraphSearch => ({
        pathFrom: typeof search.pathFrom === "string" ? search.pathFrom : undefined,
        pathTo: typeof search.pathTo === "string" ? search.pathTo : undefined,
        focus: typeof search.focus === "string" ? search.focus : undefined,
        tagTypeId: typeof search.tagTypeId === "string" ? search.tagTypeId : undefined,
        zoomLevel:
            search.zoomLevel === "overview" || search.zoomLevel === "neighborhood" || search.zoomLevel === "detail"
                ? search.zoomLevel
                : undefined,
        island: typeof search.island === "string" ? search.island : undefined
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
