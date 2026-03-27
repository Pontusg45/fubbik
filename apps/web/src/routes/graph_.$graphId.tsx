import { createFileRoute } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import { getUser } from "@/functions/get-user";

const SavedGraphView = lazy(() => import("@/features/graph/saved-graph-view"));

export const Route = createFileRoute("/graph_/$graphId")({
    component: () => (
        <Suspense
            fallback={
                <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                    <p className="text-muted-foreground">Loading saved graph...</p>
                </div>
            }
        >
            <SavedGraphView />
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
