import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ClipboardList, Plus, Upload } from "lucide-react";
import { useRef } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { PlansListContent } from "@/features/plans/plans-list-content";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

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
    const queryClient = useQueryClient();
    const fileInputRef = useRef<HTMLInputElement>(null);

    const importMutation = useMutation({
        mutationFn: async (markdown: string) => {
            return unwrapEden(
                await api.api.plans["import-markdown"].post({
                    markdown,
                })
            );
        },
        onSuccess: (data: { title?: string; steps?: unknown[] }) => {
            toast.success(
                `Imported "${data.title}" with ${data.steps?.length ?? 0} steps`
            );
            queryClient.invalidateQueries({ queryKey: ["plans"] });
        },
        onError: () => toast.error("Failed to import plan"),
    });

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <ClipboardList className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Plans</h1>
                </div>
                <div className="flex items-center gap-2">
                    <input
                        ref={fileInputRef}
                        type="file"
                        accept=".md"
                        className="hidden"
                        onChange={(e) => {
                            const file = e.target.files?.[0];
                            if (!file) return;
                            const reader = new FileReader();
                            reader.onload = () =>
                                importMutation.mutate(reader.result as string);
                            reader.readAsText(file);
                            e.target.value = "";
                        }}
                    />
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => fileInputRef.current?.click()}
                        disabled={importMutation.isPending}
                    >
                        <Upload className="mr-1 size-3.5" />
                        Import Plan
                    </Button>
                    <Button size="sm" render={<Link to="/plans/new" />}>
                        <Plus className="mr-1 size-4" />
                        New Plan
                    </Button>
                </div>
            </div>

            <PlansListContent />
        </div>
    );
}
