import { useInfiniteQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { FileText, Plus, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { useActiveSpace } from "@/features/spaces/use-active-space";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const PAGE_SIZE = 20;

export const Route = createFileRoute("/notes/")({
    component: NotesPage,
    validateSearch: (search: Record<string, unknown>): { q?: string } => ({
        q: typeof search.q === "string" && search.q.trim() ? search.q : undefined
    })
});

function NotesPage() {
    const { spaceId } = useActiveSpace();
    const navigate = useNavigate();
    const { q } = Route.useSearch();
    const debouncedSearch = useDebouncedValue(q ?? "", 250);

    function setSearch(value: string) {
        navigate({ to: "/notes", search: value.trim() ? { q: value } : {}, replace: true });
    }

    const notesQuery = useInfiniteQuery({
        queryKey: ["notes", spaceId, debouncedSearch],
        initialPageParam: 0,
        queryFn: async ({ pageParam }) =>
            unwrapEden(
                await api.api.chunks.get({
                    query: {
                        type: "note",
                        search: debouncedSearch || undefined,
                        limit: String(PAGE_SIZE),
                        offset: String(pageParam),
                        ...(spaceId === "global" ? { global: "true" } : spaceId ? { spaceId } : {})
                    }
                })
            ),
        getNextPageParam: last =>
            last.chunks.length > 0 && last.offset + last.chunks.length < last.total ? last.offset + last.chunks.length : undefined
    });

    const notes = notesQuery.data?.pages.flatMap(page => page.chunks) ?? [];
    const total = notesQuery.data?.pages[0]?.total;

    return (
        <PageContainer>
            <PageHeader
                icon={FileText}
                title="Notes"
                description="Find and revisit the notes in your knowledge base."
                count={total}
                actions={
                    <Button render={<Link to="/chunks/new" />}>
                        <Plus className="size-4" /> New note
                    </Button>
                }
            />
            <div className="mb-6 flex items-center gap-2">
                <Search className="text-muted-foreground size-4 shrink-0" />
                <Input
                    type="search"
                    aria-label="Search notes"
                    placeholder="Search notes"
                    value={q ?? ""}
                    onChange={event => setSearch(event.target.value)}
                />
            </div>

            {notesQuery.isPending ? (
                <PageLoading count={4} />
            ) : notesQuery.isError ? (
                <PageEmpty
                    variant="error"
                    error={notesQuery.error}
                    action={<Button onClick={() => notesQuery.refetch()}>Try again</Button>}
                />
            ) : notes.length === 0 ? (
                <PageEmpty
                    icon={FileText}
                    title={q ? "No matching notes" : "No notes yet"}
                    description={q ? "Try a different search." : "Create a note to capture your first idea."}
                    action={
                        q ? (
                            <Button variant="outline" onClick={() => setSearch("")}>
                                Clear search
                            </Button>
                        ) : (
                            <Button render={<Link to="/chunks/new" />}>New note</Button>
                        )
                    }
                />
            ) : (
                <>
                    <div className="space-y-3" aria-label="Notes">
                        {notes.map(note => (
                            <Card key={note.id}>
                                <CardPanel className="p-4">
                                    <Link to="/chunks/$chunkId" params={{ chunkId: note.id }} className="font-medium hover:underline">
                                        {note.title}
                                    </Link>
                                    {note.content && (
                                        <p className="text-muted-foreground mt-2 line-clamp-2 text-sm whitespace-pre-wrap">
                                            {note.content}
                                        </p>
                                    )}
                                </CardPanel>
                            </Card>
                        ))}
                    </div>
                    {notesQuery.hasNextPage && (
                        <div className="mt-6 text-center">
                            <Button variant="outline" disabled={notesQuery.isFetchingNextPage} onClick={() => notesQuery.fetchNextPage()}>
                                Load more notes
                            </Button>
                        </div>
                    )}
                </>
            )}
        </PageContainer>
    );
}
