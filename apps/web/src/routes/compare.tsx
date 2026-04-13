import { useQuery } from "@tanstack/react-query";
import { createFileRoute, useSearch } from "@tanstack/react-router";
import { Columns2 } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { MarkdownRenderer } from "@/components/markdown-renderer";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/compare")({
    component: ComparePage,
    validateSearch: (search: Record<string, unknown>) => ({
        left: (search.left as string) ?? undefined,
        right: (search.right as string) ?? undefined
    }),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

interface ChunkSummary {
    id: string;
    title: string;
    type: string;
    content: string;
    tags?: Array<{ name: string }>;
}

function simpleDiff(left: string[], right: string[]): { left: ("same" | "removed")[]; right: ("same" | "added")[] } {
    const maxLen = Math.max(left.length, right.length);
    const leftResult: ("same" | "removed")[] = [];
    const rightResult: ("same" | "added")[] = [];
    for (let i = 0; i < maxLen; i++) {
        if (left[i] === right[i]) {
            leftResult.push("same");
            rightResult.push("same");
        } else {
            if (i < left.length) leftResult.push("removed");
            if (i < right.length) rightResult.push("added");
        }
    }
    return { left: leftResult, right: rightResult };
}

function ChunkSelector({
    value,
    onChange,
    chunks,
    label
}: {
    value: string | undefined;
    onChange: (id: string) => void;
    chunks: ChunkSummary[];
    label: string;
}) {
    const [search, setSearch] = useState("");
    const filtered = chunks.filter(c => c.title.toLowerCase().includes(search.toLowerCase()));

    return (
        <div className="space-y-2">
            <label className="text-sm font-medium">{label}</label>
            <input
                type="text"
                placeholder="Search chunks..."
                value={search}
                onChange={e => setSearch(e.target.value)}
                className="border-input bg-background w-full rounded-md border px-3 py-2 text-sm"
            />
            {search && (
                <div className="max-h-48 overflow-y-auto rounded-md border">
                    {filtered.slice(0, 20).map(c => (
                        <button
                            key={c.id}
                            onClick={() => {
                                onChange(c.id);
                                setSearch("");
                            }}
                            className={`hover:bg-muted w-full px-3 py-2 text-left text-sm ${c.id === value ? "bg-primary/10" : ""}`}
                        >
                            {c.title}
                        </button>
                    ))}
                </div>
            )}
            {value && !search && (
                <p className="text-muted-foreground text-xs">
                    Selected: {chunks.find(c => c.id === value)?.title ?? value}
                </p>
            )}
        </div>
    );
}

function DiffView({ leftContent, rightContent }: { leftContent: string; rightContent: string }) {
    const { leftLines, rightLines, diff } = useMemo(() => {
        const l = (leftContent ?? "").split("\n");
        const r = (rightContent ?? "").split("\n");
        return { leftLines: l, rightLines: r, diff: simpleDiff(l, r) };
    }, [leftContent, rightContent]);

    const bgClass = (status: string) => {
        if (status === "removed") return "bg-red-500/10";
        if (status === "added") return "bg-green-500/10";
        return "";
    };

    return (
        <div className="grid grid-cols-2 gap-4">
            <pre className="overflow-x-auto rounded-md border p-3 text-xs">
                {leftLines.map((line, i) => (
                    <div key={i} className={bgClass(diff.left[i] ?? "same")}>
                        {line || "\u00A0"}
                    </div>
                ))}
            </pre>
            <pre className="overflow-x-auto rounded-md border p-3 text-xs">
                {rightLines.map((line, i) => (
                    <div key={i} className={bgClass(diff.right[i] ?? "same")}>
                        {line || "\u00A0"}
                    </div>
                ))}
            </pre>
        </div>
    );
}

function ChunkPanel({ chunk }: { chunk: ChunkSummary }) {
    return (
        <div className="space-y-3">
            <div>
                <h3 className="text-lg font-semibold">{chunk.title}</h3>
                <div className="mt-1 flex items-center gap-2">
                    <Badge variant="outline">{chunk.type}</Badge>
                    {chunk.tags?.map(t => (
                        <Badge key={t.name} variant="secondary" className="text-xs">
                            {t.name}
                        </Badge>
                    ))}
                </div>
            </div>
            <div className="prose prose-sm dark:prose-invert max-w-none">
                <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
            </div>
        </div>
    );
}

function ComparePage() {
    const search = useSearch({ from: "/compare" });
    const [leftId, setLeftId] = useState<string | undefined>(search.left);
    const [rightId, setRightId] = useState<string | undefined>(search.right);
    const [showDiff, setShowDiff] = useState(false);

    const chunksQuery = useQuery({
        queryKey: ["chunks-list-compare"],
        queryFn: async () => {
            const res = unwrapEden(await api.api.chunks.get({ query: { limit: "100" } }));
            return (res.chunks ?? []) as ChunkSummary[];
        }
    });

    const leftQuery = useQuery({
        queryKey: ["chunk", leftId],
        queryFn: async () => {
            const res = unwrapEden(await api.api.chunks({ id: leftId! }).get()) as any;
            return (res.chunk ?? res) as ChunkSummary;
        },
        enabled: !!leftId
    });

    const rightQuery = useQuery({
        queryKey: ["chunk", rightId],
        queryFn: async () => {
            const res = unwrapEden(await api.api.chunks({ id: rightId! }).get()) as any;
            return (res.chunk ?? res) as ChunkSummary;
        },
        enabled: !!rightId
    });

    const allChunks = chunksQuery.data ?? [];

    return (
        <PageContainer maxWidth="6xl">
            <PageHeader icon={Columns2} title="Compare Chunks" />

            <div className="mb-6 grid grid-cols-2 gap-6">
                <ChunkSelector value={leftId} onChange={setLeftId} chunks={allChunks} label="Left chunk" />
                <ChunkSelector value={rightId} onChange={setRightId} chunks={allChunks} label="Right chunk" />
            </div>

            {(leftQuery.isLoading || rightQuery.isLoading) && <PageLoading count={3} />}

            {leftQuery.data && rightQuery.data && (
                <>
                    <div className="mb-4 flex items-center gap-3">
                        <button
                            onClick={() => setShowDiff(d => !d)}
                            className="text-sm font-medium underline"
                        >
                            {showDiff ? "Show rendered" : "Show diff"}
                        </button>
                    </div>

                    {showDiff ? (
                        <DiffView
                            leftContent={leftQuery.data.content}
                            rightContent={rightQuery.data.content}
                        />
                    ) : (
                        <div className="grid grid-cols-2 gap-6">
                            <Card>
                                <CardPanel className="p-6">
                                    <ChunkPanel chunk={leftQuery.data} />
                                </CardPanel>
                            </Card>
                            <Card>
                                <CardPanel className="p-6">
                                    <ChunkPanel chunk={rightQuery.data} />
                                </CardPanel>
                            </Card>
                        </div>
                    )}
                </>
            )}
        </PageContainer>
    );
}
