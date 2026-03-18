import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Check, ChevronDown, ChevronRight, ExternalLink } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { BackLink } from "@/components/back-link";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { AssumptionResolver } from "@/features/reviews/assumption-resolver";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/reviews_/$sessionId")({
    component: ReviewDetailPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

const statusConfig: Record<string, { label: string; variant: "default" | "secondary" | "outline" }> = {
    in_progress: { label: "In Progress", variant: "default" },
    completed: { label: "Completed", variant: "secondary" },
    reviewed: { label: "Reviewed", variant: "outline" }
};

const REQ_STATUS_STYLES: Record<string, string> = {
    passing: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
    failing: "bg-red-500/10 text-red-600 border-red-500/30 dark:text-red-400",
    untested: "bg-muted text-muted-foreground"
};

interface SessionData {
    session: {
        id: string;
        title: string;
        status: string;
        prUrl: string | null;
        reviewBrief: string | null;
        createdAt: string;
        completedAt: string | null;
        reviewedAt: string | null;
    };
    chunkRefs: Array<{ chunkId: string; chunkTitle: string; reason: string }>;
    assumptions: Array<{ id: string; description: string; resolved: boolean; resolution: string | null }>;
    requirementRefs: Array<{
        requirementId: string;
        requirementTitle: string;
        requirementStatus: string;
        totalSteps: number;
        stepsAddressed: number;
    }>;
    allRequirements: Array<{ id: string; title: string; status: string; steps: unknown[] }>;
    allConventions: Array<{ id: string; title: string }>;
}

function ReviewDetailPage() {
    const { sessionId } = Route.useParams();
    const queryClient = useQueryClient();

    const [showNotAddressed, setShowNotAddressed] = useState(false);
    const [showNotChecked, setShowNotChecked] = useState(false);
    const [showReviewForm, setShowReviewForm] = useState(false);
    const [reqStatuses, setReqStatuses] = useState<Record<string, string>>({});

    const { data, isLoading, error } = useQuery({
        queryKey: ["session", sessionId],
        queryFn: async () => {
            const res = await (api.api.sessions as any)({ id: sessionId }).get();
            if (res.error) throw new Error("Failed to fetch session");
            return res.data as SessionData;
        }
    });

    const reviewMutation = useMutation({
        mutationFn: async () => {
            const requirementStatuses = Object.entries(reqStatuses).map(([requirementId, status]) => ({
                requirementId,
                status: status as "passing" | "failing" | "untested"
            }));
            return unwrapEden(
                await (api.api.sessions as any)({ id: sessionId }).review.patch({
                    requirementStatuses: requirementStatuses.length > 0 ? requirementStatuses : undefined
                })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["session", sessionId] });
            queryClient.invalidateQueries({ queryKey: ["sessions"] });
            setShowReviewForm(false);
            toast.success("Session marked as reviewed");
        },
        onError: () => toast.error("Failed to review session")
    });

    if (isLoading) {
        return (
            <Shell>
                <p className="text-muted-foreground py-12 text-center text-sm">Loading...</p>
            </Shell>
        );
    }

    if (error || !data) {
        return (
            <Shell>
                <p className="text-muted-foreground py-12 text-center text-sm">Session not found.</p>
            </Shell>
        );
    }

    const { session, chunkRefs, assumptions, requirementRefs, allRequirements, allConventions } = data;
    const statusInfo = statusConfig[session.status] ?? { label: session.status, variant: "secondary" as const };

    const formattedDate = new Date(session.createdAt).toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric"
    });

    const addressedReqIds = new Set(requirementRefs.map(r => r.requirementId));
    const notAddressed = allRequirements.filter(r => !addressedReqIds.has(r.id));

    const referencedChunkIds = new Set(chunkRefs.map(c => c.chunkId));
    const notChecked = allConventions.filter(c => !referencedChunkIds.has(c.id));

    function handleStartReview() {
        const initial: Record<string, string> = {};
        for (const ref of requirementRefs) {
            initial[ref.requirementId] = ref.requirementStatus ?? "untested";
        }
        setReqStatuses(initial);
        setShowReviewForm(true);
    }

    return (
        <Shell>
            {/* Header */}
            <div className="mb-6">
                <div className="flex items-start justify-between gap-4">
                    <div>
                        <h1 className="text-2xl font-bold tracking-tight">{session.title}</h1>
                        <div className="mt-2 flex flex-wrap items-center gap-2">
                            <Badge variant={statusInfo.variant}>{statusInfo.label}</Badge>
                            <span className="text-muted-foreground text-xs">{formattedDate}</span>
                            {session.prUrl && (
                                <a
                                    href={session.prUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs transition-colors"
                                >
                                    <ExternalLink className="size-3" />
                                    PR
                                </a>
                            )}
                        </div>
                    </div>
                </div>
            </div>

            {/* Summary stats */}
            <div className="mb-8 grid grid-cols-3 gap-4">
                <div className="rounded-lg border p-4 text-center">
                    <p className="text-2xl font-bold">{requirementRefs.length}/{allRequirements.length}</p>
                    <p className="text-muted-foreground text-xs">Requirements addressed</p>
                </div>
                <div className="rounded-lg border p-4 text-center">
                    <p className="text-2xl font-bold">{chunkRefs.length}</p>
                    <p className="text-muted-foreground text-xs">Chunks referenced</p>
                </div>
                <div className="rounded-lg border p-4 text-center">
                    <p className="text-2xl font-bold">{assumptions.length}</p>
                    <p className="text-muted-foreground text-xs">Assumptions</p>
                </div>
            </div>

            {/* Requirements Addressed */}
            <div className="mb-6">
                <h2 className="mb-3 text-sm font-semibold">Requirements Addressed</h2>
                {requirementRefs.length > 0 ? (
                    <div className="space-y-1">
                        {requirementRefs.map(ref => (
                            <Link
                                key={ref.requirementId}
                                to={"/requirements/$requirementId" as string}
                                params={{ requirementId: ref.requirementId } as Record<string, string>}
                                className="hover:bg-muted/50 flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                            >
                                <span>{ref.requirementTitle}</span>
                                <div className="flex items-center gap-2">
                                    <span className="text-muted-foreground text-xs">
                                        {ref.stepsAddressed}/{ref.totalSteps} steps
                                    </span>
                                    <Badge variant="outline" className={REQ_STATUS_STYLES[ref.requirementStatus] ?? REQ_STATUS_STYLES.untested}>
                                        {ref.requirementStatus}
                                    </Badge>
                                </div>
                            </Link>
                        ))}
                    </div>
                ) : (
                    <p className="text-muted-foreground text-sm">No requirements addressed yet.</p>
                )}

                {notAddressed.length > 0 && (
                    <div className="mt-3">
                        <button
                            onClick={() => setShowNotAddressed(!showNotAddressed)}
                            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs font-medium transition-colors"
                        >
                            {showNotAddressed ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                            Not addressed ({notAddressed.length})
                        </button>
                        {showNotAddressed && (
                            <div className="mt-2 space-y-1">
                                {notAddressed.map(req => (
                                    <Link
                                        key={req.id}
                                        to={"/requirements/$requirementId" as string}
                                        params={{ requirementId: req.id } as Record<string, string>}
                                        className="hover:bg-muted/50 text-muted-foreground flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                                    >
                                        <span>{req.title}</span>
                                        <Badge variant="outline" className={REQ_STATUS_STYLES[req.status] ?? REQ_STATUS_STYLES.untested}>
                                            {req.status}
                                        </Badge>
                                    </Link>
                                ))}
                            </div>
                        )}
                    </div>
                )}
            </div>

            {/* Conventions Applied */}
            <div className="mb-6">
                <h2 className="mb-3 text-sm font-semibold">Conventions Applied</h2>
                {chunkRefs.length > 0 ? (
                    <div className="space-y-1">
                        {chunkRefs.map(ref => (
                            <Link
                                key={ref.chunkId}
                                to="/chunks/$chunkId"
                                params={{ chunkId: ref.chunkId }}
                                className="hover:bg-muted/50 block rounded-md border px-3 py-2 transition-colors"
                            >
                                <span className="text-sm font-medium">{ref.chunkTitle}</span>
                                <p className="text-muted-foreground mt-0.5 text-xs">{ref.reason}</p>
                            </Link>
                        ))}
                    </div>
                ) : (
                    <p className="text-muted-foreground text-sm">No conventions applied yet.</p>
                )}

                {notChecked.length > 0 && (
                    <div className="mt-3">
                        <button
                            onClick={() => setShowNotChecked(!showNotChecked)}
                            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs font-medium transition-colors"
                        >
                            {showNotChecked ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                            Not checked ({notChecked.length})
                        </button>
                        {showNotChecked && (
                            <div className="mt-2 space-y-1">
                                {notChecked.map(conv => (
                                    <Link
                                        key={conv.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: conv.id }}
                                        className="hover:bg-muted/50 text-muted-foreground block rounded-md border px-3 py-2 text-sm transition-colors"
                                    >
                                        {conv.title}
                                    </Link>
                                ))}
                            </div>
                        )}
                    </div>
                )}
            </div>

            {/* Assumptions */}
            {assumptions.length > 0 && (
                <div className="mb-6">
                    <h2 className="mb-3 text-sm font-semibold">Assumptions</h2>
                    <div className="space-y-2">
                        {assumptions.map(assumption => (
                            <AssumptionResolver key={assumption.id} assumption={assumption} sessionId={sessionId} />
                        ))}
                    </div>
                </div>
            )}

            {/* Review action */}
            {session.status === "completed" && !showReviewForm && (
                <div className="border-t pt-6">
                    <Button onClick={handleStartReview}>
                        <Check className="mr-1 size-3.5" />
                        Mark as Reviewed
                    </Button>
                </div>
            )}

            {showReviewForm && (
                <div className="border-t pt-6">
                    <h2 className="mb-3 text-sm font-semibold">Review Requirements</h2>
                    <div className="space-y-2">
                        {requirementRefs.map(ref => (
                            <div key={ref.requirementId} className="flex items-center justify-between rounded-md border px-3 py-2">
                                <span className="text-sm">{ref.requirementTitle}</span>
                                <select
                                    value={reqStatuses[ref.requirementId] ?? "untested"}
                                    onChange={e =>
                                        setReqStatuses(prev => ({ ...prev, [ref.requirementId]: e.target.value }))
                                    }
                                    className="bg-background focus:ring-ring rounded-md border px-2 py-1 text-sm focus:ring-2 focus:outline-none"
                                >
                                    <option value="passing">Passing</option>
                                    <option value="failing">Failing</option>
                                    <option value="untested">Untested</option>
                                </select>
                            </div>
                        ))}
                    </div>
                    <div className="mt-4 flex justify-end gap-2">
                        <Button
                            variant="outline"
                            onClick={() => setShowReviewForm(false)}
                        >
                            Cancel
                        </Button>
                        <Button
                            onClick={() => reviewMutation.mutate()}
                            disabled={reviewMutation.isPending}
                        >
                            {reviewMutation.isPending ? "Submitting..." : "Submit Review"}
                        </Button>
                    </div>
                </div>
            )}
        </Shell>
    );
}

function Shell({ children }: { children: React.ReactNode }) {
    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <BackLink to="/reviews" label="Reviews" />
            {children}
        </div>
    );
}
