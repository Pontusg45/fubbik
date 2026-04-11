import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { BookOpen } from "lucide-react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/learn")({
    component: LearnPage,
});

function LearnPage() {
    const { data } = useQuery({
        queryKey: ["learning-paths"],
        queryFn: async () => unwrapEden(await (api.api as any)["learning-paths"].get()),
    });

    const paths = ((data as any) ?? []) as Array<{
        id: string;
        title: string;
        description?: string;
        chunkIds: string[];
    }>;

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            <h1 className="mb-6 text-2xl font-bold tracking-tight">Learning Paths</h1>
            {paths.length === 0 ? (
                <div className="rounded-lg border p-8 text-center">
                    <BookOpen className="mx-auto size-8 text-muted-foreground/40 mb-2" />
                    <p className="text-muted-foreground">No learning paths yet.</p>
                </div>
            ) : (
                <div className="space-y-3">
                    {paths.map(p => (
                        <Link
                            key={p.id}
                            to="/learn/$pathId"
                            params={{ pathId: p.id }}
                            className="flex items-start gap-3 rounded-lg border bg-card p-4 hover:bg-muted/50 transition-colors"
                        >
                            <BookOpen className="mt-0.5 size-5 text-muted-foreground shrink-0" />
                            <div className="min-w-0 flex-1">
                                <div className="font-semibold">{p.title}</div>
                                {p.description && (
                                    <p className="mt-1 text-sm text-muted-foreground">{p.description}</p>
                                )}
                                <p className="mt-2 text-xs text-muted-foreground">
                                    {p.chunkIds.length} chunks in sequence
                                </p>
                            </div>
                        </Link>
                    ))}
                </div>
            )}
        </div>
    );
}
