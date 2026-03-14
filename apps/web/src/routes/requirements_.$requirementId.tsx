import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Clipboard, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/requirements_/$requirementId")({
    component: RequirementDetail,
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

type Status = "passing" | "failing" | "untested";

function statusColor(status: string) {
    switch (status) {
        case "passing":
            return "text-green-600 bg-green-500/10 border-green-500/30";
        case "failing":
            return "text-red-600 bg-red-500/10 border-red-500/30";
        default:
            return "text-muted-foreground bg-muted";
    }
}

function priorityLabel(priority: string) {
    switch (priority) {
        case "must":
            return "Must";
        case "should":
            return "Should";
        case "could":
            return "Could";
        case "wont":
            return "Won't";
        default:
            return priority;
    }
}

function keywordColor(keyword: string) {
    switch (keyword) {
        case "given":
            return "text-blue-600 dark:text-blue-400";
        case "when":
            return "text-amber-600 dark:text-amber-400";
        case "then":
            return "text-green-600 dark:text-green-400";
        case "and":
        case "but":
            return "text-muted-foreground";
        default:
            return "";
    }
}

function RequirementDetail() {
    const { requirementId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();

    const { data, isLoading, error } = useQuery({
        queryKey: ["requirement", requirementId],
        queryFn: async () => {
            return unwrapEden(await api.api.requirements({ id: requirementId }).get());
        }
    });

    const statusMutation = useMutation({
        mutationFn: async (status: Status) => {
            return unwrapEden(await api.api.requirements({ id: requirementId }).status.patch({ status }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirement", requirementId] });
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
            toast.success("Status updated");
        },
        onError: () => {
            toast.error("Failed to update status");
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.requirements({ id: requirementId }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
            toast.success("Requirement deleted");
            navigate({ to: "/requirements" });
        },
        onError: () => {
            toast.error("Failed to delete requirement");
        }
    });

    async function handleExport(format: "gherkin" | "vitest" | "markdown") {
        try {
            const text = unwrapEden(
                await api.api.requirements({ id: requirementId }).export.get({ query: { format } })
            ) as string;
            await navigator.clipboard.writeText(text);
            toast.success(`${format.charAt(0).toUpperCase() + format.slice(1)} copied to clipboard`);
        } catch {
            toast.error("Failed to export");
        }
    }

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-5xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/requirements" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Loading...</p>
                </div>
            </div>
        );
    }

    if (error || !data) {
        return (
            <div className="container mx-auto max-w-5xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/requirements" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Requirement not found.</p>
                </div>
            </div>
        );
    }

    const req = data as Record<string, unknown>;
    const title = req.title as string;
    const description = req.description as string | null;
    const status = (req.status as string) ?? "untested";
    const priority = req.priority as string | null;
    const steps = (req.steps as Array<{ keyword: string; text: string }>) ?? [];
    const chunks = (req.chunks as Array<{ id: string; title: string }>) ?? [];
    const warnings = (req as Record<string, unknown>).warnings as Array<{ step: number; warning: string }> | undefined;

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/requirements" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="flex gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        className="text-destructive"
                        onClick={() => {
                            if (confirm(`Delete "${title}"?`)) deleteMutation.mutate();
                        }}
                        disabled={deleteMutation.isPending}
                    >
                        <Trash2 className="size-3.5" />
                        {deleteMutation.isPending ? "Deleting..." : "Delete"}
                    </Button>
                </div>
            </div>

            {/* Header */}
            <div className="mb-6">
                <div className="mb-2 flex items-center gap-2">
                    <h1 className="text-2xl font-bold tracking-tight">{title}</h1>
                    <Badge variant="outline" className={statusColor(status)}>
                        {status}
                    </Badge>
                    {priority && (
                        <Badge variant="secondary">{priorityLabel(priority)}</Badge>
                    )}
                </div>
                {description && (
                    <p className="text-muted-foreground text-sm">{description}</p>
                )}
            </div>

            {/* Status toggle */}
            <div className="mb-6 flex items-center gap-2">
                <span className="text-sm font-medium">Status:</span>
                {(["passing", "failing", "untested"] as const).map(s => (
                    <Button
                        key={s}
                        variant={status === s ? "default" : "outline"}
                        size="sm"
                        onClick={() => statusMutation.mutate(s)}
                        disabled={statusMutation.isPending}
                        className={status === s ? "" : ""}
                    >
                        {s.charAt(0).toUpperCase() + s.slice(1)}
                    </Button>
                ))}
            </div>

            <Separator className="my-6" />

            {/* Steps display */}
            <div className="mb-6">
                <h2 className="mb-3 text-sm font-semibold">Steps</h2>
                <Card>
                    <CardPanel className="p-4">
                        <div className="space-y-1.5 font-mono text-sm">
                            {steps.map((step, i) => (
                                <div key={i} className="flex gap-2">
                                    <span className={`font-bold uppercase ${keywordColor(step.keyword)}`}>
                                        {step.keyword}
                                    </span>
                                    <span>{step.text}</span>
                                </div>
                            ))}
                        </div>
                    </CardPanel>
                </Card>
            </div>

            {/* Warnings */}
            {warnings && warnings.length > 0 && (
                <div className="mb-6 rounded-md border border-yellow-500/30 bg-yellow-500/10 p-4">
                    <h3 className="mb-2 text-sm font-semibold text-yellow-600 dark:text-yellow-400">
                        Cross-reference Warnings
                    </h3>
                    <ul className="space-y-1">
                        {warnings.map((w, i) => (
                            <li key={i} className="text-sm text-yellow-600 dark:text-yellow-400">
                                {w.step >= 0 ? `Step ${w.step + 1}: ` : ""}{w.warning}
                            </li>
                        ))}
                    </ul>
                </div>
            )}

            {/* Linked chunks */}
            {chunks.length > 0 && (
                <>
                    <Separator className="my-6" />
                    <div className="mb-6">
                        <h2 className="mb-3 text-sm font-semibold">Linked Chunks</h2>
                        <div className="space-y-2">
                            {chunks.map(c => (
                                <Link
                                    key={c.id}
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: c.id }}
                                    className="hover:bg-muted block rounded-md border px-3 py-2 text-sm font-medium transition-colors"
                                >
                                    {c.title}
                                </Link>
                            ))}
                        </div>
                    </div>
                </>
            )}

            <Separator className="my-6" />

            {/* Export buttons */}
            <div>
                <h2 className="mb-3 text-sm font-semibold">Export</h2>
                <div className="flex gap-2">
                    <Button variant="outline" size="sm" onClick={() => handleExport("gherkin")}>
                        <Clipboard className="mr-1 size-3.5" />
                        Gherkin
                    </Button>
                    <Button variant="outline" size="sm" onClick={() => handleExport("vitest")}>
                        <Clipboard className="mr-1 size-3.5" />
                        Vitest
                    </Button>
                    <Button variant="outline" size="sm" onClick={() => handleExport("markdown")}>
                        <Clipboard className="mr-1 size-3.5" />
                        Markdown
                    </Button>
                </div>
            </div>
        </div>
    );
}
