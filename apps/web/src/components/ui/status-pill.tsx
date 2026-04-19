import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

/**
 * Shared visual shell for "status" style pills. Callers map their own
 * vocabulary (plan status, task status, health bucket, requirement status…)
 * onto one of these variants and pass a label. This is deliberately thin —
 * domain pills keep their semantic names at the module level and just use
 * this for consistent styling.
 */
export type StatusPillVariant =
    | "slate"
    | "blue"
    | "indigo"
    | "amber"
    | "emerald"
    | "zinc"
    | "red"
    | "violet"
    | "green"
    | "yellow"
    | "orange";

const VARIANT_CLASSES: Record<StatusPillVariant, string> = {
    slate: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    blue: "bg-blue-500/15 border-blue-500/30 text-blue-400",
    indigo: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    amber: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    emerald: "bg-emerald-500/15 border-emerald-500/30 text-emerald-400",
    zinc: "bg-zinc-500/15 border-zinc-500/30 text-zinc-500",
    red: "bg-red-500/15 border-red-500/30 text-red-400",
    violet: "bg-violet-500/15 border-violet-500/30 text-violet-400",
    green: "bg-green-500/15 border-green-500/30 text-green-400",
    yellow: "bg-yellow-500/15 border-yellow-500/30 text-yellow-400",
    orange: "bg-orange-500/15 border-orange-500/30 text-orange-400",
};

export interface StatusPillProps {
    variant: StatusPillVariant;
    label?: string;
    className?: string;
    children?: ReactNode;
    /** Optional tiny dot icon shown before the label. */
    dot?: boolean;
}

export function StatusPill({ variant, label, className, children, dot }: StatusPillProps) {
    return (
        <span
            className={cn(
                "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                VARIANT_CLASSES[variant],
                className,
            )}
        >
            {dot && <span className="size-1.5 rounded-full bg-current opacity-70" />}
            {children ?? label}
        </span>
    );
}
