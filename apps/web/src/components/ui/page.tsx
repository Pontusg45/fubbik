import { AlertCircle, FileQuestion, Filter, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { Badge } from "@/components/ui/badge";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { Empty, EmptyAction, EmptyDescription, EmptyMedia, EmptyTitle } from "@/components/ui/empty";

// ─── Page Container ───

interface PageContainerProps {
    children: ReactNode;
    maxWidth?: "3xl" | "4xl" | "5xl" | "6xl";
    className?: string;
}

const widthMap = {
    "3xl": "max-w-3xl",
    "4xl": "max-w-4xl",
    "5xl": "max-w-5xl",
    "6xl": "max-w-6xl",
};

export function PageContainer({ children, maxWidth = "3xl", className }: PageContainerProps) {
    return (
        <div className={`container mx-auto ${widthMap[maxWidth]} px-4 py-8 ${className ?? ""}`}>
            {children}
        </div>
    );
}

// ─── Page Header ───

interface PageHeaderProps {
    icon?: LucideIcon;
    title: string;
    description?: string;
    count?: number;
    actions?: ReactNode;
}

export function PageHeader({ icon: Icon, title, description, count, actions }: PageHeaderProps) {
    return (
        <div className="mb-6 flex items-center justify-between">
            <div>
                <div className="flex items-center gap-2">
                    {Icon && <Icon className="size-5" />}
                    <h1 className="text-2xl font-bold tracking-tight">{title}</h1>
                    {count != null && (
                        <Badge variant="secondary" className="ml-1">
                            {count}
                        </Badge>
                    )}
                </div>
                {description && (
                    <p className="text-muted-foreground mt-1 text-sm">{description}</p>
                )}
            </div>
            {actions && <div className="flex items-center gap-2">{actions}</div>}
        </div>
    );
}

// ─── Page Loading State ───

interface PageLoadingProps {
    count?: number;
}

export function PageLoading({ count = 5 }: PageLoadingProps) {
    return <SkeletonList count={count} />;
}

// ─── Page Empty State ───

/**
 * Variants:
 *   "none"     — nothing exists yet (default). Show an onboarding CTA.
 *   "filtered" — data exists but the current filters hide it. Show a "Clear filters" action.
 *   "error"    — the fetch failed. Show "Try again" + the error message if any.
 * Extra props give sensible defaults per variant so page-level callers only
 * override the title/description when the defaults don't fit.
 */
type PageEmptyVariant = "none" | "filtered" | "error";

interface PageEmptyProps {
    icon?: LucideIcon;
    title?: string;
    description?: string;
    action?: ReactNode;
    variant?: PageEmptyVariant;
    /** For variant="filtered": a one-click handler to clear every active filter. */
    onReset?: () => void;
    /** For variant="error": the underlying error, rendered as description fallback. */
    error?: { message?: string } | null;
}

export function PageEmpty({
    icon,
    title,
    description,
    action,
    variant = "none",
    onReset,
    error,
}: PageEmptyProps) {
    const Icon = icon ?? defaultIcon(variant);
    const t = title ?? defaultTitle(variant);
    const d = description ?? defaultDescription(variant, error);
    const a = action ?? defaultAction(variant, onReset);
    return (
        <Empty>
            <EmptyMedia variant="icon">
                <Icon className="h-10 w-10" />
            </EmptyMedia>
            <EmptyTitle>{t}</EmptyTitle>
            <EmptyDescription>{d}</EmptyDescription>
            {a && <EmptyAction>{a}</EmptyAction>}
        </Empty>
    );
}

function defaultIcon(variant: PageEmptyVariant): LucideIcon {
    switch (variant) {
        case "filtered": return Filter;
        case "error": return AlertCircle;
        default: return FileQuestion;
    }
}

function defaultTitle(variant: PageEmptyVariant): string {
    switch (variant) {
        case "filtered": return "No results";
        case "error": return "Something went wrong";
        default: return "Nothing here yet";
    }
}

function defaultDescription(variant: PageEmptyVariant, error?: { message?: string } | null): string {
    switch (variant) {
        case "filtered": return "No items match the current filters. Try clearing them to see everything.";
        case "error": return error?.message ?? "We couldn't load this. Try again in a moment.";
        default: return "";
    }
}

function defaultAction(variant: PageEmptyVariant, onReset?: () => void): ReactNode {
    if (variant === "filtered" && onReset) {
        return (
            <button
                type="button"
                onClick={onReset}
                className="text-sm underline underline-offset-2 hover:opacity-80"
            >
                Clear filters
            </button>
        );
    }
    return undefined;
}
