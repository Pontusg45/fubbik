import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { AlphabeticalIndex } from "@/features/browse/alphabetical-index";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/browse")({
    component: BrowsePage,
});

function BrowsePage() {
    const { data } = useQuery({
        queryKey: ["browse-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
    });

    const chunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string }>;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-6">
                <h1 className="text-2xl font-bold tracking-tight">Browse</h1>
                <p className="text-muted-foreground text-sm">All chunks in alphabetical order</p>
            </div>
            <AlphabeticalIndex chunks={chunks} />
        </div>
    );
}
