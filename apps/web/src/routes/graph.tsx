import { createFileRoute } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import { RouteErrorBoundary } from "@/components/route-error-boundary";
import { getUser } from "@/functions/get-user";

const GraphView = lazy(() => import("@/features/graph/graph-view"));

export interface GraphSearch {
    pathFrom?: string;
    pathTo?: string;
    tags?: string;
    types?: string;
    focus?: string;
    depth?: number;
    groupBy?: "tag" | "type" | "codebase" | "none";
    all?: number;
}

export const Route = createFileRoute("/graph")({
    validateSearch: (search: Record<string, unknown>): GraphSearch => ({
        pathFrom: typeof search.pathFrom === "string" ? search.pathFrom : undefined,
        pathTo: typeof search.pathTo === "string" ? search.pathTo : undefined,
        tags: typeof search.tags === "string" ? search.tags : undefined,
        types: typeof search.types === "string" ? search.types : undefined,
        focus: typeof search.focus === "string" ? search.focus : undefined,
        depth: typeof search.depth === "number" ? search.depth : undefined,
        groupBy:
            search.groupBy === "tag" || search.groupBy === "type" || search.groupBy === "codebase" || search.groupBy === "none"
                ? (search.groupBy as "tag" | "type" | "codebase" | "none")
                : undefined,
        all: typeof search.all === "number" ? search.all : undefined
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
