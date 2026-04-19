import { MoreHorizontal } from "lucide-react";
import type { ReactNode } from "react";

import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

export interface RowActionItem {
    /** Stable key for React reconciliation. */
    key: string;
    /** Rendered body — typically `<> <Icon /> Label </>`. */
    children: ReactNode;
    /** Click handler. Receives the click event so callers can stopPropagation. */
    onSelect?: (e?: React.MouseEvent) => void;
    /** Renders as a destructive (red) item when true. */
    destructive?: boolean;
    /** Renders a divider above this item. Use for grouping. */
    separatorBefore?: boolean;
    /** Disabled state. */
    disabled?: boolean;
}

export interface RowActionsMenuProps {
    items: RowActionItem[];
    /** Accessible label on the trigger button. */
    ariaLabel: string;
    /** Align the popover edge. Default: "end" (right-aligned, best for row kebabs). */
    align?: "start" | "center" | "end";
    /** Custom trigger classes — the default is the fade-in-on-group-hover pattern. */
    triggerClassName?: string;
}

/**
 * Standard kebab-menu for list rows. Replaces the hand-rolled
 * MoreHorizontal → DropdownMenu pattern used across /tags, /plans, and
 * (henceforth) /chunks, /requirements, /codebases, /workspaces.
 *
 * The `items` API intentionally keeps children as ReactNode so callers can
 * embed icons and badges without wrestling with a stricter `{icon, label}`
 * shape. The destructive + separatorBefore flags cover the common case of
 * "Edit / Archive / Duplicate — Delete".
 */
export function RowActionsMenu({
    items,
    ariaLabel,
    align = "end",
    triggerClassName,
}: RowActionsMenuProps) {
    return (
        <DropdownMenu>
            <DropdownMenuTrigger
                render={
                    <button
                        aria-label={ariaLabel}
                        className={cn(
                            "text-muted-foreground/0 group-hover:text-muted-foreground hover:text-foreground rounded p-1 transition-colors",
                            triggerClassName,
                        )}
                        onClick={e => e.stopPropagation()}
                    >
                        <MoreHorizontal className="size-4" />
                    </button>
                }
            />
            <DropdownMenuContent align={align}>
                {items.map(item => (
                    <RowItem key={item.key} item={item} />
                ))}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}

function RowItem({ item }: { item: RowActionItem }) {
    return (
        <>
            {item.separatorBefore && <DropdownMenuSeparator />}
            <DropdownMenuItem
                onClick={item.onSelect}
                disabled={item.disabled}
                className={cn(item.destructive && "text-destructive")}
            >
                {item.children}
            </DropdownMenuItem>
        </>
    );
}
