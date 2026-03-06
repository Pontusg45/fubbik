import { useQuery } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { Tags } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";

export const Route = createFileRoute("/tags")({
    component: TagsPage,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});

function TagsPage() {
    const tagsQuery = useQuery({
        queryKey: ["tags"],
        queryFn: async () => {
            const { data, error } = await api.api.tags.get();
            if (error) return [];
            return data as Exclude<typeof data, { message: string }>;
        }
    });

    const tags = Array.isArray(tagsQuery.data) ? tagsQuery.data : [];

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center gap-2">
                <Tags className="size-5" />
                <h1 className="text-2xl font-bold tracking-tight">Tags</h1>
                <Badge variant="secondary" className="ml-2">
                    {tags.length}
                </Badge>
            </div>

            <Card>
                <CardPanel className="p-6">
                    {tagsQuery.isLoading ? (
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    ) : tags.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No tags yet.</p>
                    ) : (
                        <div className="flex flex-wrap gap-2">
                            {tags.map(t => (
                                <Badge key={t.tag} variant="outline" className="text-sm hover:bg-muted cursor-pointer transition-colors">
                                    {t.tag}
                                    <span className="text-muted-foreground ml-1.5 font-mono text-xs">{t.count}</span>
                                </Badge>
                            ))}
                        </div>
                    )}
                </CardPanel>
            </Card>
        </div>
    );
}
