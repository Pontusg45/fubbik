import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { AlertTriangle } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { useActiveSpace } from "@/features/spaces/use-active-space";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const STATUS_STYLES: Record<string, string> = {
    passing: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
    failing: "bg-red-500/10 text-red-600 border-red-500/30 dark:text-red-400",
    untested: "bg-muted text-muted-foreground"
};

// `planSteps` and `sessions` come back as genuinely untyped empty arrays:
// Node's `getTraceabilityMatrix` hard-codes both to `[]` under its own TODO
// from the plans rewrite, so no backend populates them and there is no
// element shape to publish — Rust's DTO mirrors that as `unknown[]`
// (`crates/fubbik-api/src/coverage/dto.rs:113-127`). These describe the shape
// the render code below is written against for when that lands. The narrowing
// casts at the two map sites are the only place this shape is asserted; the
// row's scalar fields are checked against the generated API types as normal.
// `sessions` needs no element type here — it is only ever counted, never read
// into, so it stays `unknown[]` rather than asserting a shape nothing checks.
interface PlanStepRef {
    stepId: string;
    stepDescription: string;
    stepStatus: string;
    planId: string;
    planTitle: string;
    planStatus: string;
}


export function TraceabilityContent() {
    const { spaceId } = useActiveSpace();

    const traceQuery = useQuery({
        queryKey: ["traceability", spaceId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (spaceId) query.codebaseId = spaceId;
            return unwrapEden(await api.api.requirements.traceability.get({ query }));
        }
    });

    if (traceQuery.isLoading) {
        return <p className="text-muted-foreground">Loading traceability data...</p>;
    }

    const rows = traceQuery.data ?? [];
    const withPlan = rows.filter(r => r.planSteps.length > 0);
    const withSession = rows.filter(r => r.sessions.length > 0);
    const gaps = rows.filter(r => r.planSteps.length === 0 && r.sessions.length === 0);

    return (
        <>
            {/* Summary stats */}
            <div className="mb-6 grid grid-cols-4 gap-4">
                <Card>
                    <CardPanel className="text-center">
                        <p className="text-muted-foreground text-xs font-medium uppercase">Requirements</p>
                        <p className="text-3xl font-bold">{rows.length}</p>
                    </CardPanel>
                </Card>
                <Card>
                    <CardPanel className="text-center">
                        <p className="text-xs font-medium text-blue-600 uppercase">With Plan</p>
                        <p className="text-3xl font-bold text-blue-600">{withPlan.length}</p>
                    </CardPanel>
                </Card>
                <Card>
                    <CardPanel className="text-center">
                        <p className="text-xs font-medium text-purple-600 uppercase">With Session</p>
                        <p className="text-3xl font-bold text-purple-600">{withSession.length}</p>
                    </CardPanel>
                </Card>
                <Card>
                    <CardPanel className="text-center">
                        <p className="text-xs font-medium text-amber-600 uppercase">Gaps</p>
                        <p className="text-3xl font-bold text-amber-600">{gaps.length}</p>
                    </CardPanel>
                </Card>
            </div>

            {/* Requirement rows */}
            <div className="space-y-3">
                {rows.map(req => {
                    // Group plan steps by plan
                    const planGroups = new Map<
                        string,
                        { planId: string; planTitle: string; planStatus: string; steps: PlanStepRef[] }
                    >();
                    for (const step of req.planSteps as PlanStepRef[]) {
                        if (!planGroups.has(step.planId)) {
                            planGroups.set(step.planId, {
                                planId: step.planId,
                                planTitle: step.planTitle,
                                planStatus: step.planStatus,
                                steps: []
                            });
                        }
                        planGroups.get(step.planId)!.steps.push(step);
                    }

                    const hasGap = req.planSteps.length === 0 && req.sessions.length === 0;

                    return (
                        <Card key={req.id} className={hasGap ? "border-amber-500/30" : ""}>
                            <CardPanel className="p-4">
                                <div className="flex items-start justify-between">
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2">
                                            <Link
                                                to="/requirements/$requirementId"
                                                params={{ requirementId: req.id }}
                                                className="font-medium hover:underline"
                                            >
                                                {req.title}
                                            </Link>
                                            <Badge variant="outline" className={STATUS_STYLES[req.status] ?? STATUS_STYLES.untested}>
                                                {req.status}
                                            </Badge>
                                            {req.priority && (
                                                <Badge variant="secondary" className="text-xs">
                                                    {req.priority}
                                                </Badge>
                                            )}
                                        </div>

                                        <div className="mt-2 flex flex-wrap gap-4 text-sm">
                                            {/* Plan coverage */}
                                            {planGroups.size > 0 ? (
                                                Array.from(planGroups.values()).map(pg => {
                                                    const done = pg.steps.filter(s => s.stepStatus === "done").length;
                                                    return (
                                                        <span key={pg.planId} className="text-muted-foreground">
                                                            Covered by{" "}
                                                            <Link
                                                                to="/plans/$planId"
                                                                params={{ planId: pg.planId }}
                                                                className="text-foreground hover:underline"
                                                            >
                                                                {pg.planTitle}
                                                            </Link>{" "}
                                                            ({done}/{pg.steps.length} steps done)
                                                        </span>
                                                    );
                                                })
                                            ) : (
                                                <span className="text-muted-foreground italic">No plan</span>
                                            )}

                                            {/* Session coverage */}
                                            <span className="text-muted-foreground italic">Not addressed</span>
                                        </div>
                                    </div>

                                    {hasGap && <AlertTriangle className="mt-1 size-4 shrink-0 text-amber-500" />}
                                </div>
                            </CardPanel>
                        </Card>
                    );
                })}

                {rows.length === 0 && (
                    <p className="text-muted-foreground py-8 text-center text-sm">
                        No requirements found. Create requirements to track traceability.
                    </p>
                )}
            </div>
        </>
    );
}
