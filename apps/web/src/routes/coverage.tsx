import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckCircle, XCircle, BarChart3 } from "lucide-react";
import { useState } from "react";

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

    const coverageQuery = useQuery({
        queryKey: ["coverage", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await (api.api.requirements as any).coverage.get({ query }));
        }
    });

    const data = coverageQuery.data as
        | {
              covered: { id: string; title: string; requirementCount: number }[];
              uncovered: { id: string; title: string }[];
              stats: { total: number; covered: number; uncovered: number; percentage: number };
          }
        | undefined;

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
