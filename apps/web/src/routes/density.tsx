import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, Folder, FolderTree, Loader2 } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { PageContainer, PageHeader } from "@/components/ui/page";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/density")({
    component: DensityPage,
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

type DensityNode = {
    name: string;
    path: string;
    chunkCount: number;
    directChunkCount: number;
    children: DensityNode[];
    chunks: Array<{ id: string; title: string; type: string; source: "applies_to" | "file_ref" }>;
};

function heatClass(count: number, max: number): string {
    if (max === 0) return "bg-muted/30";
    const ratio = count / max;
    if (ratio >= 0.75) return "bg-emerald-600/25";
    if (ratio >= 0.5) return "bg-emerald-500/20";
    if (ratio >= 0.25) return "bg-emerald-500/15";
    if (ratio > 0) return "bg-emerald-500/10";
    return "bg-muted/30";
}

function DensityPage() {
    const { codebaseId } = useActiveCodebase();

    const densityQuery = useQuery({
        queryKey: ["density", codebaseId],
        queryFn: async () => {
            const query: Record<string, string> = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.density.get({ query }));
        }
    });

    const tree = densityQuery.data?.tree as DensityNode | undefined;
    const totals = densityQuery.data?.totals;

    const maxCount = useMemo(() => {
        if (!tree) return 0;
        let max = 0;
        function walk(n: DensityNode) {
            if (n.chunkCount > max) max = n.chunkCount;
            for (const c of n.children) walk(c);
        }
        walk(tree);
        return max;
    }, [tree]);

    return (
        <PageContainer maxWidth="5xl">
            <PageHeader
                icon={FolderTree}
                title="Knowledge density map"
                description="Where your chunks land in the filesystem. Darker = more coverage."
            />

            {totals && (
                <Card className="mb-4">
                    <CardPanel className="flex items-center gap-6 p-4 text-sm">
                        <div>
                            <span className="text-muted-foreground">Chunks covered</span>{" "}
                            <span className="font-semibold">{totals.chunksCovered}</span>
                        </div>
                        <div>
                            <span className="text-muted-foreground">Paths tracked</span>{" "}
                            <span className="font-semibold">{totals.pathsTracked}</span>
                        </div>
                    </CardPanel>
                </Card>
            )}

            <Card>
                {densityQuery.isLoading ? (
                    <CardPanel className="flex items-center justify-center p-8">
                        <Loader2 className="text-muted-foreground size-5 animate-spin" />
                    </CardPanel>
                ) : !tree || tree.children.length === 0 ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">
                            No file coverage yet. Add file references or applies-to patterns to your chunks.
                        </p>
                    </CardPanel>
                ) : (
                    <CardPanel className="p-2">
                        <ul className="space-y-0.5">
                            {tree.children.map(child => (
                                <TreeNode key={child.path} node={child} depth={0} maxCount={maxCount} />
                            ))}
                        </ul>
                    </CardPanel>
                )}
            </Card>
        </PageContainer>
    );
}

function TreeNode({ node, depth, maxCount }: { node: DensityNode; depth: number; maxCount: number }) {
    const [expanded, setExpanded] = useState(depth < 1);
    const hasChildren = node.children.length > 0;
    const hasDirect = node.chunks.length > 0;

    return (
        <li>
            <div
                className={`flex items-center gap-2 rounded px-1.5 py-1 ${heatClass(node.chunkCount, maxCount)}`}
                style={{ paddingLeft: `${depth * 16 + 6}px` }}
            >
                {hasChildren ? (
                    <button
                        type="button"
                        onClick={() => setExpanded(e => !e)}
                        className="hover:bg-background/60 flex size-4 items-center justify-center rounded"
                        aria-label={expanded ? "Collapse" : "Expand"}
                    >
                        {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                    </button>
                ) : (
                    <span className="size-4" />
                )}
                <Folder className="text-muted-foreground size-3.5 shrink-0" />
                <span className="truncate font-mono text-sm">{node.name}</span>
                <Badge variant="secondary" size="sm" className="ml-auto shrink-0">
                    {node.chunkCount}
                </Badge>
            </div>
            {expanded && (
                <>
                    {hasDirect && (
                        <ul className="mt-0.5 space-y-0.5" style={{ paddingLeft: `${(depth + 1) * 16 + 6}px` }}>
                            {node.chunks.map(c => (
                                <li key={`${c.id}-${c.source}`}>
                                    <Link
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: c.id }}
                                        className="hover:bg-muted/60 flex items-center gap-1.5 rounded px-1.5 py-0.5 text-xs"
                                    >
                                        <span className="truncate">{c.title}</span>
                                        <Badge variant="outline" size="sm" className="ml-auto shrink-0">
                                            {c.source === "applies_to" ? "glob" : "ref"}
                                        </Badge>
                                    </Link>
                                </li>
                            ))}
                        </ul>
                    )}
                    {hasChildren && (
                        <ul className="mt-0.5 space-y-0.5">
                            {node.children.map(child => (
                                <TreeNode key={child.path} node={child} depth={depth + 1} maxCount={maxCount} />
                            ))}
                        </ul>
                    )}
                </>
            )}
        </li>
    );
}
