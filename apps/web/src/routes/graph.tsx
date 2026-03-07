import { createFileRoute } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import { getUser } from "@/functions/get-user";

const GraphView = lazy(() => import("@/features/graph/graph-view"));

export const Route = createFileRoute("/graph")({
    component: () => (
        <Suspense fallback={
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <p className="text-muted-foreground">Loading graph...</p>
            </div>
        }>
            <GraphView />
        </Suspense>
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
