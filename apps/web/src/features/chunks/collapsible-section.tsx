import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { LucideIcon } from "lucide-react";

interface CollapsibleSectionProps {
    title: string;
    icon?: LucideIcon;
    count?: number;
    defaultOpen?: boolean;
    children: React.ReactNode;
}

export function CollapsibleSection({ title, icon: Icon, count, defaultOpen = false, children }: CollapsibleSectionProps) {
    const [open, setOpen] = useState(defaultOpen);
    return (
        <div className="border-t py-3">
            <button
                onClick={() => setOpen(!open)}
                className="flex w-full items-center gap-2 text-sm font-medium hover:text-foreground text-muted-foreground transition-colors"
            >
                {open ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                {Icon && <Icon className="size-4" />}
                {title}
                {count != null && (
                    <span className="ml-auto rounded-full bg-muted px-2 py-0.5 text-xs">{count}</span>
                )}
            </button>
            {open && <div className="mt-3">{children}</div>}
        </div>
    );
}
