import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { CheckCircle, Circle, Clock, SkipForward, Ban } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const STEP_STATUS_ICON: Record<string, React.ReactNode> = {
    done: <CheckCircle className="size-3.5 text-emerald-500" />,
    in_progress: <Clock className="size-3.5 text-blue-500" />,
    pending: <Circle className="size-3.5 text-muted-foreground" />,
    skipped: <SkipForward className="size-3.5 text-muted-foreground" />,
    blocked: <Ban className="size-3.5 text-red-500" />
};

const PLAN_STATUS_STYLES: Record<string, string> = {
    draft: "bg-muted text-muted-foreground",
    active: "border-blue-500/30 bg-blue-500/10 text-blue-600 dark:text-blue-400",
    completed: "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    archived: "bg-muted text-muted-foreground"
};

interface PlanWithSteps {
    id: string;
    title: string;
    status: string;
    steps: Array<{
        id: string;
        description: string;
        status: string;
        requirementId: string | null;
    }>;
}

export function RequirementPlans({ requirementId }: { requirementId: string }) {
    const plansQuery = useQuery({
        queryKey: ["plans"],
        queryFn: async () => {
            return unwrapEden(await api.api.plans.get({ query: {} })) as Array<{
                id: string;
                title: string;
                status: string;
            }>;
        },
        staleTime: 60_000
    });

    const planDetailsQuery = useQuery({
        queryKey: ["plan-details-for-requirement", requirementId, plansQuery.data?.map(p => p.id)],
        queryFn: async () => {
            const plans = plansQuery.data ?? [];
            const results: PlanWithSteps[] = [];
            for (const plan of plans) {
                try {
                    const detail = unwrapEden(
                        await api.api.plans({ id: plan.id }).get()
                    ) as {
                        id: string;
                        title: string;
                        status: string;
                        steps: Array<{
                            id: string;
                            description: string;
                            status: string;
                            requirementId: string | null;
                        }>;
                    };
                    const matchingSteps = detail.steps.filter(
                        s => s.requirementId === requirementId
                    );
                    if (matchingSteps.length > 0) {
                        results.push({
                            id: detail.id,
                            title: detail.title,
                            status: detail.status,
                            steps: matchingSteps
                        });
                    }
                } catch {
                    // skip plans we can't fetch
                }
            }
            return results;
        },
        enabled: !!plansQuery.data && plansQuery.data.length > 0,
        staleTime: 60_000
    });

    const linkedPlans = planDetailsQuery.data;

    if (!linkedPlans || linkedPlans.length === 0) return null;

    return (
        <div className="mb-6">
            <h2 className="mb-3 text-sm font-semibold">Linked Plans</h2>
            <div className="space-y-3">
                {linkedPlans.map(plan => (
                    <div key={plan.id} className="rounded-lg border p-3">
                        <div className="mb-2 flex items-center justify-between">
                            <Link
                                to="/plans/$planId"
                                params={{ planId: plan.id }}
                                className="text-sm font-medium hover:underline"
                            >
                                {plan.title}
                            </Link>
                            <Badge
                                variant="outline"
                                className={PLAN_STATUS_STYLES[plan.status] ?? ""}
                            >
                                {plan.status}
                            </Badge>
                        </div>
                        <div className="space-y-1">
                            {plan.steps.map(step => (
                                <div
                                    key={step.id}
                                    className="flex items-center gap-2 text-sm"
                                >
                                    {STEP_STATUS_ICON[step.status] ?? STEP_STATUS_ICON.pending}
                                    <span className={step.status === "done" ? "text-muted-foreground line-through" : ""}>
                                        {step.description}
                                    </span>
                                </div>
                            ))}
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
