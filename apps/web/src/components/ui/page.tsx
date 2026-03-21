import type { LucideIcon } from "lucide-react";
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

interface PageEmptyProps {
    icon: LucideIcon;
    title: string;
    description: string;
    action?: ReactNode;
}

export function PageEmpty({ icon: Icon, title, description, action }: PageEmptyProps) {
    return (
        <Empty>
            <EmptyMedia variant="icon">
                <Icon className="h-10 w-10" />
            </EmptyMedia>
            <EmptyTitle>{title}</EmptyTitle>
            <EmptyDescription>{description}</EmptyDescription>
            {action && <EmptyAction>{action}</EmptyAction>}
        </Empty>
    );
}
