import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckCircle, XCircle, BarChart3, Grid3x3 } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/coverage")({
    component: CoveragePage,
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

function CoveragePage() {
    const { codebaseId } = useActiveCodebase();
    const [showCovered, setShowCovered] = useState(false);
    const [showMatrix, setShowMatrix] = useState(false);

    const coverageQuery = useQuery({
        queryKey: ["coverage", codebaseId, showMatrix],
        queryFn: async () => {
            const query: { codebaseId?: string; detail?: string } = {};
            if (codebaseId) query.codebaseId = codebaseId;
            if (showMatrix) query.detail = "true";
            return unwrapEden(await (api.api.requirements as any).coverage.get({ query }));
        }
    });

    const data = coverageQuery.data as
        | {
              covered: { id: string; title: string; requirementCount: number }[];
              uncovered: { id: string; title: string }[];
              stats: { total: number; covered: number; uncovered: number; percentage: number };
              matrix?: Array<{
                  chunkId: string;
                  chunkTitle: string;
                  requirementId: string;
                  requirementTitle: string;
                  requirementStatus: string;
              }>;
          }
        | undefined;

    const uniqueRequirements = useMemo(() => {
        if (!data?.matrix) return [];
        const map = new Map<string, { id: string; title: string }>();
        for (const pair of data.matrix) {
            map.set(pair.requirementId, { id: pair.requirementId, title: pair.requirementTitle });
        }
        return Array.from(map.values());
    }, [data?.matrix]);

    const allChunks = useMemo(() => {
        if (!data) return [];
        return [
            ...data.covered.map(c => ({ id: c.id, title: c.title })),
            ...data.uncovered.map(c => ({ id: c.id, title: c.title }))
        ];
    }, [data]);

    const coverageMap = useMemo(() => {
        if (!data?.matrix) return new Map<string, Set<string>>();
        const map = new Map<string, Set<string>>();
        for (const pair of data.matrix) {
            if (!map.has(pair.chunkId)) map.set(pair.chunkId, new Set());
            map.get(pair.chunkId)!.add(pair.requirementId);
        }
        return map;
    }, [data?.matrix]);

    const isCovered = (chunkId: string) =>
        coverageMap.has(chunkId) && coverageMap.get(chunkId)!.size > 0;

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6">
                <h1 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
                    <BarChart3 className="size-6" />
                    Requirement Coverage
                </h1>
                <p className="text-muted-foreground mt-1 text-sm">
                    Which chunks are linked to requirements and which are not.
                </p>
            </div>

            {coverageQuery.isLoading && (
                <p className="text-muted-foreground">Loading coverage data...</p>
            )}

            {data && (
                <>
                    <div className="mb-8 grid grid-cols-4 gap-4">
                        <Card>
                            <CardPanel className="text-center">
                                <p className="text-muted-foreground text-xs font-medium uppercase">Total</p>
                                <p className="text-3xl font-bold">{data.stats.total}</p>
                            </CardPanel>
                        </Card>
                        <Card>
                            <CardPanel className="text-center">
                                <p className="text-xs font-medium uppercase text-green-600">Covered</p>
                                <p className="text-3xl font-bold text-green-600">{data.stats.covered}</p>
                            </CardPanel>
                        </Card>
                        <Card>
                            <CardPanel className="text-center">
                                <p className="text-xs font-medium uppercase text-red-600">Uncovered</p>
                                <p className="text-3xl font-bold text-red-600">{data.stats.uncovered}</p>
                            </CardPanel>
                        </Card>
                        <Card>
                            <CardPanel className="text-center">
                                <p className="text-muted-foreground text-xs font-medium uppercase">Coverage</p>
                                <p className="text-3xl font-bold">{data.stats.percentage}%</p>
                            </CardPanel>
                        </Card>
                    </div>

                    <div className="mb-4 h-3 w-full overflow-hidden rounded-full bg-red-100 dark:bg-red-900/30">
                        <div
                            className="h-full rounded-full bg-green-500 transition-all"
                            style={{ width: `${data.stats.percentage}%` }}
                        />
                    </div>

                    <div className="mb-4 flex items-center justify-between">
                        <Button
                            variant={showMatrix ? "default" : "outline"}
                            size="sm"
                            onClick={() => setShowMatrix(!showMatrix)}
                        >
                            <Grid3x3 className="mr-1 size-4" />
                            {showMatrix ? "Hide Matrix" : "Show Matrix"}
                        </Button>
                    </div>

                    {showMatrix && data?.matrix && (
                        <Card className="mt-6">
                            <CardPanel className="overflow-x-auto p-0">
                                <table className="w-full text-sm">
                                    <thead>
                                        <tr className="border-b">
                                            <th className="bg-background sticky left-0 z-10 min-w-[200px] px-4 py-2 text-left font-medium">
                                                Chunk
                                            </th>
                                            {uniqueRequirements.map(req => (
                                                <th
                                                    key={req.id}
                                                    className="min-w-[100px] px-3 py-2 text-center"
                                                >
                                                    <Link
                                                        to="/requirements/$requirementId"
                                                        params={{ requirementId: req.id }}
                                                        className="block max-w-[120px] truncate text-xs font-medium hover:underline"
                                                    >
                                                        {req.title}
                                                    </Link>
                                                </th>
                                            ))}
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {allChunks.map(chunk => (
                                            <tr
                                                key={chunk.id}
                                                className={
                                                    isCovered(chunk.id) ? "" : "bg-amber-500/5"
                                                }
                                            >
                                                <td className="bg-background sticky left-0 z-10 border-t px-4 py-2">
                                                    <Link
                                                        to="/chunks/$chunkId"
                                                        params={{ chunkId: chunk.id }}
                                                        className="block max-w-[250px] truncate hover:underline"
                                                    >
                                                        {chunk.title}
                                                    </Link>
                                                </td>
                                                {uniqueRequirements.map(req => (
                                                    <td
                                                        key={req.id}
                                                        className="border-t px-3 py-2 text-center"
                                                    >
                                                        {coverageMap.get(chunk.id)?.has(req.id) ? (
                                                            <CheckCircle className="inline size-4 text-emerald-500" />
                                                        ) : null}
                                                    </td>
                                                ))}
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </CardPanel>
                        </Card>
                    )}

                    <div className="space-y-6">
                        <Card>
                            <CardHeader>
                                <div className="flex items-center justify-between">
                                    <CardTitle className="flex items-center gap-2 text-sm">
                                        <XCircle className="size-4 text-red-500" />
                                        Uncovered ({data.uncovered.length})
                                    </CardTitle>
                                </div>
                            </CardHeader>
                            <CardPanel className="space-y-1 pt-0">
                                {data.uncovered.length === 0 ? (
                                    <p className="text-muted-foreground text-sm">All chunks are covered!</p>
                                ) : (
                                    data.uncovered.map(c => (
                                        <div
                                            key={c.id}
                                            className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                                        >
                                            <Link
                                                to="/chunks/$chunkId"
                                                params={{ chunkId: c.id }}
                                                className="hover:underline"
                                            >
                                                {c.title}
                                            </Link>
                                        </div>
                                    ))
                                )}
                            </CardPanel>
                        </Card>

                        <Card>
                            <CardHeader>
                                <div className="flex items-center justify-between">
                                    <CardTitle className="flex items-center gap-2 text-sm">
                                        <CheckCircle className="size-4 text-green-500" />
                                        Covered ({data.covered.length})
                                    </CardTitle>
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => setShowCovered(!showCovered)}
                                    >
                                        {showCovered ? "Hide" : "Show"}
                                    </Button>
                                </div>
                            </CardHeader>
                            {showCovered && (
                                <CardPanel className="space-y-1 pt-0">
                                    {data.covered.length === 0 ? (
                                        <p className="text-muted-foreground text-sm">No chunks are covered yet.</p>
                                    ) : (
                                        data.covered.map(c => (
                                            <div
                                                key={c.id}
                                                className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                                            >
                                                <Link
                                                    to="/chunks/$chunkId"
                                                    params={{ chunkId: c.id }}
                                                    className="hover:underline"
                                                >
                                                    {c.title}
                                                </Link>
                                                <Badge variant="secondary" size="sm">
                                                    {c.requirementCount} req{c.requirementCount !== 1 ? "s" : ""}
                                                </Badge>
                                            </div>
                                        ))
                                    )}
                                </CardPanel>
                            )}
                        </Card>
                    </div>
                </>
            )}
        </div>
    );
}
