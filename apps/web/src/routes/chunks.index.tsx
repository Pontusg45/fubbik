import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, redirect, useNavigate } from "@tanstack/react-router";
import { Clock, Plus, Search } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/chunks/")({
    component: ChunksList,
    validateSearch: (search: Record<string, unknown>) => ({
        type: (search.type as string) || undefined,
        q: (search.q as string) || undefined,
        page: Number(search.page) || 1
    }),
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});

function ChunksList() {
    const navigate = useNavigate({ from: "/chunks/" });
    const { type, q, page } = Route.useSearch();
    const [searchInput, setSearchInput] = useState(q ?? "");
    const limit = 20;
    const offset = ((page ?? 1) - 1) * limit;

    const chunksQuery = useQuery({
        queryKey: ["chunks-list", type, q, page],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: {
                            type,
                            search: q,
                            limit: String(limit),
                            offset: String(offset)
                        }
                    })
                );
            } catch {
                return null;
            }
        }
    });

    const chunks = chunksQuery.data?.chunks ?? [];
    const total = chunksQuery.data?.total ?? 0;
    const totalPages = Math.ceil(total / limit);

    const types = ["note", "document", "reference", "schema", "checklist"];

    function updateSearch(params: Partial<{ type: string; q: string; page: number }>) {
        navigate({
            search: {
                type: params.type !== undefined ? params.type : type,
                q: params.q !== undefined ? params.q : q,
                page: params.page ?? 1
            }
        });
    }

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <h1 className="text-2xl font-bold tracking-tight">Chunks</h1>
                <Button render={<Link to="/chunks/new" />}>
                    <Plus className="size-4" />
                    New Chunk
                </Button>
            </div>

            {/* Search & filters */}
            <div className="mb-6 flex flex-wrap items-center gap-3">
                <div className="relative flex-1">
                    <Search className="text-muted-foreground absolute left-3 top-1/2 size-4 -translate-y-1/2" />
                    <input
                        type="text"
                        value={searchInput}
                        onChange={e => setSearchInput(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter") updateSearch({ q: searchInput || undefined });
                        }}
                        placeholder="Search chunks..."
                        className="bg-background focus:ring-ring w-full rounded-md border py-2 pl-9 pr-3 text-sm focus:ring-2 focus:outline-none"
                    />
                </div>
                <div className="flex gap-1">
                    <Button
                        variant={!type ? "default" : "outline"}
                        size="sm"
                        onClick={() => updateSearch({ type: undefined })}
                    >
                        All
                    </Button>
                    {types.map(t => (
                        <Button
                            key={t}
                            variant={type === t ? "default" : "outline"}
                            size="sm"
                            onClick={() => updateSearch({ type: type === t ? undefined : t })}
                        >
                            {t}
                        </Button>
                    ))}
                </div>
            </div>

            {/* Results */}
            <Card>
                {chunksQuery.isLoading ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    </CardPanel>
                ) : chunks.length === 0 ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">No chunks found.</p>
                    </CardPanel>
                ) : (
                    chunks.map((chunk, i) => (
                        <div key={chunk.id}>
                            {i > 0 && <Separator />}
                            <Link to="/chunks/$chunkId" params={{ chunkId: chunk.id }} className="block">
                                <CardPanel className="hover:bg-muted/50 flex items-center justify-between gap-4 p-4 transition-colors">
                                    <div className="min-w-0">
                                        <p className="truncate text-sm font-medium">{chunk.title}</p>
                                        <div className="mt-1 flex items-center gap-2">
                                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                {chunk.type}
                                            </Badge>
                                            {(chunk.tags as string[]).map(tag => (
                                                <Badge key={tag} variant="outline" size="sm" className="text-[10px]">
                                                    {tag}
                                                </Badge>
                                            ))}
                                        </div>
                                    </div>
                                    <span className="text-muted-foreground flex shrink-0 items-center gap-1 text-xs">
                                        <Clock className="size-3" />
                                        {new Date(chunk.updatedAt).toLocaleDateString()}
                                    </span>
                                </CardPanel>
                            </Link>
                        </div>
                    ))
                )}
            </Card>

            {/* Pagination */}
            {totalPages > 1 && (
                <div className="mt-4 flex items-center justify-center gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        disabled={page <= 1}
                        onClick={() => updateSearch({ page: page - 1 })}
                    >
                        Previous
                    </Button>
                    <span className="text-muted-foreground text-sm">
                        Page {page} of {totalPages}
                    </span>
                    <Button
                        variant="outline"
                        size="sm"
                        disabled={page >= totalPages}
                        onClick={() => updateSearch({ page: page + 1 })}
                    >
                        Next
                    </Button>
                </div>
            )}
        </div>
    );
}
