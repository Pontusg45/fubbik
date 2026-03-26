import { createFileRoute, Link } from "@tanstack/react-router";
import { ClipboardList, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { PlansListContent } from "@/features/plans/plans-list-content";
import { getUser } from "@/functions/get-user";

export const Route = createFileRoute("/plans/")({
    component: PlansPage,
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

function PlansPage() {
    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <ClipboardList className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Plans</h1>
                </div>
                <Button size="sm" render={<Link to="/plans/new" />}>
                    <Plus className="mr-1 size-4" />
                    New Plan
                </Button>
            </div>

            <PlansListContent />
        </div>
    );
}
