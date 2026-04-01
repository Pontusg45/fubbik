import { Link } from "@tanstack/react-router";
import { ArrowRight, Check, FileText, GitBranch, Import, X } from "lucide-react";
import { useCallback, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

interface MilestoneCardsProps {
    stats: { chunks?: number; connections?: number; conventions?: number; appliesToCount?: number } | null;
}

interface Milestone {
    id: string;
    title: string;
    description: string;
    icon: typeof FileText;
    check: (stats: NonNullable<MilestoneCardsProps["stats"]>) => boolean;
    linkTo: string;
    linkLabel: string;
}

const MILESTONES: Milestone[] = [
    {
        id: "first-convention",
        title: "Add your first convention",
        description: "Document a coding convention or pattern",
        icon: FileText,
        check: (stats) => (stats.conventions ?? 0) > 0,
        linkTo: "/chunks/new",
        linkLabel: "Create chunk",
    },
    {
        id: "connect-chunks",
        title: "Connect two chunks",
        description: "Link related knowledge together",
        icon: GitBranch,
        check: (stats) => (stats.connections ?? 0) > 0,
        linkTo: "/graph",
        linkLabel: "Open graph",
    },
    {
        id: "import-docs",
        title: "Import documentation",
        description: "Build up your knowledge base",
        icon: Import,
        check: (stats) => (stats.chunks ?? 0) > 10,
        linkTo: "/chunks/new",
        linkLabel: "Import",
    },
    {
        id: "file-patterns",
        title: "Set up file patterns",
        description: "Link chunks to file areas with globs",
        icon: FileText,
        check: (stats) => (stats.appliesToCount ?? 0) > 0,
        linkTo: "/chunks",
        linkLabel: "Browse chunks",
    },
];

const STORAGE_KEY = "fubbik-milestones-dismissed";

function getDismissedIds(): Set<string> {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) return new Set();
        return new Set(JSON.parse(raw));
    } catch {
        return new Set();
    }
}

function saveDismissedIds(ids: Set<string>) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...ids]));
}

export function MilestoneCards({ stats }: MilestoneCardsProps) {
    const [dismissedIds, setDismissedIds] = useState(() => getDismissedIds());

    const dismiss = useCallback((id: string) => {
        setDismissedIds(prev => {
            const next = new Set(prev);
            next.add(id);
            saveDismissedIds(next);
            return next;
        });
    }, []);

    if (!stats) return null;

    const visibleMilestones = MILESTONES.filter(m => !dismissedIds.has(m.id));

    if (visibleMilestones.length === 0) return null;

    return (
        <div className="mb-8">
            <h2 className="text-muted-foreground mb-3 text-xs font-medium uppercase tracking-wide">Getting Started</h2>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                {visibleMilestones.map(milestone => {
                    const completed = milestone.check(stats);
                    const Icon = milestone.icon;

                    return (
                        <div
                            key={milestone.id}
                            className="bg-card relative rounded-lg border p-3"
                        >
                            <button
                                type="button"
                                onClick={() => dismiss(milestone.id)}
                                className="text-muted-foreground hover:text-foreground absolute top-1.5 right-1.5 rounded p-0.5 transition-colors"
                                aria-label={`Dismiss ${milestone.title}`}
                            >
                                <X className="size-3" />
                            </button>
                            <div className="flex items-center gap-2">
                                {completed ? (
                                    <div className="flex size-5 shrink-0 items-center justify-center rounded-full bg-emerald-500/10">
                                        <Check className="size-3 text-emerald-500" />
                                    </div>
                                ) : (
                                    <Icon className="text-muted-foreground size-4 shrink-0" />
                                )}
                                <span className="truncate text-xs font-medium">{milestone.title}</span>
                            </div>
                            <p className="text-muted-foreground mt-1.5 text-[11px] leading-tight">{milestone.description}</p>
                            {completed ? (
                                <Badge variant="secondary" size="sm" className="mt-2 text-[10px] text-emerald-600 dark:text-emerald-400">
                                    Complete
                                </Badge>
                            ) : (
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className="mt-2 h-6 px-2 text-[11px]"
                                    render={<Link to={milestone.linkTo as any} />}
                                >
                                    {milestone.linkLabel}
                                    <ArrowRight className="ml-1 size-3" />
                                </Button>
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
