import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { FileSearch, Search } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/context")({
    component: ContextPage,
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

interface ContextResult {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: string;
}

const typeVariant: Record<string, "default" | "secondary" | "outline"> = {
    note: "secondary",
    document: "default",
    reference: "outline",
    schema: "outline",
    checklist: "secondary"
};

function ExpandableContent({ content }: { content: string }) {
    const [expanded, setExpanded] = useState(false);
    const preview = content.slice(0, 200);
    const needsTruncation = content.length > 200;

    return (
        <div className="mt-2 text-sm">
            <pre className="whitespace-pre-wrap font-sans">
                {expanded ? content : preview}
                {needsTruncation && !expanded && "..."}
            </pre>
            {needsTruncation && (
                <button
                    type="button"
                    onClick={() => setExpanded(!expanded)}
                    className="text-muted-foreground hover:text-foreground mt-1 text-xs underline"
                >
                    {expanded ? "Show less" : "Show more"}
                </button>
            )}
        </div>
    );
}

function ContextPage() {
    const { codebaseId } = useActiveCodebase();
    const [inputValue, setInputValue] = useState("");
    const [searchPath, setSearchPath] = useState("");

    const contextQuery = useQuery({
        queryKey: ["context-for-file", searchPath, codebaseId],
        queryFn: async () => {
            const result = unwrapEden(
                await api.api.context["for-file"].get({
                    query: {
                        path: searchPath,
                        ...(codebaseId && codebaseId !== "global" ? { codebaseId } : {})
                    }
                })
            );
            return (result.chunks ?? []) as ContextResult[];
        },
        enabled: searchPath.length > 0
    });

    const results = Array.isArray(contextQuery.data) ? contextQuery.data : [];

    function handleSearch(e: React.FormEvent) {
        e.preventDefault();
        if (inputValue.trim()) {
            setSearchPath(inputValue.trim());
        }
    }

    return (
        <PageContainer>
            <PageHeader
                icon={FileSearch}
                title="Context"
                description="Find relevant knowledge chunks for a file path"
                count={searchPath ? results.length : undefined}
            />

            <form onSubmit={handleSearch} className="mb-6 flex gap-2">
                <Input
                    type="text"
                    placeholder="Enter a file path, e.g. src/utils/api.ts"
                    value={inputValue}
                    onChange={e => setInputValue(e.target.value)}
                    className="flex-1"
                />
                <Button type="submit" disabled={!inputValue.trim()}>
                    <Search className="mr-1.5 size-4" />
                    Search
                </Button>
            </form>

            <Card>
                <CardPanel className="p-6">
                    {!searchPath ? (
                        <PageEmpty
                            icon={FileSearch}
                            title="Enter a file path"
                            description="Enter a file path to find relevant knowledge"
                        />
                    ) : contextQuery.isLoading ? (
                        <PageLoading count={4} />
                    ) : results.length === 0 ? (
                        <PageEmpty
                            icon={FileSearch}
                            title="No results"
                            description={`No chunks found matching "${searchPath}"`}
                        />
                    ) : (
                        <div className="divide-y">
                            {results.map(result => (
                                <div key={result.id} className="py-4 first:pt-0 last:pb-0">
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0 flex-1">
                                            <div className="flex flex-wrap items-center gap-2">
                                                <Link
                                                    to="/chunks/$chunkId"
                                                    params={{ chunkId: result.id }}
                                                    className="text-foreground hover:underline font-medium"
                                                >
                                                    {result.title}
                                                </Link>
                                                <Badge variant={typeVariant[result.type] ?? "secondary"} size="sm">
                                                    {result.type}
                                                </Badge>
                                                <Badge variant="outline" size="sm">
                                                    {result.matchReason}
                                                </Badge>
                                            </div>
                                            {result.summary && (
                                                <blockquote className="text-muted-foreground mt-2 border-l-2 pl-3 text-sm italic">
                                                    {result.summary}
                                                </blockquote>
                                            )}
                                            <ExpandableContent content={result.content} />
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardPanel>
            </Card>
        </PageContainer>
    );
}
