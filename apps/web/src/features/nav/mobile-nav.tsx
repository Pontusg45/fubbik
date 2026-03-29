import { Link } from "@tanstack/react-router";
import {
    Activity,
    Blocks,
    BookOpen,
    ClipboardCheck,
    ClipboardList,
    Clock,
    FileSearch,
    FileText,
    FolderGit2,
    FolderUp,
    Layers,
    LayoutDashboard,
    Menu,
    Network,
    Search,
    Settings,
    Tags
} from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet";

const primaryItems = [
    { label: "Dashboard", to: "/dashboard" as const, icon: LayoutDashboard },
    { label: "Chunks", to: "/chunks" as const, icon: Blocks },
    { label: "Graph", to: "/graph" as const, icon: Network },
    { label: "Requirements", to: "/requirements" as const, icon: ClipboardCheck },
    { label: "Reviews", to: "/reviews" as const, icon: ClipboardList },
    { label: "Docs", to: "/docs" as const, icon: FileText },
    { label: "Search", to: "/search" as const, icon: Search }
];

const manageItems = [
    { label: "Tags", to: "/tags" as const, icon: Tags },
    { label: "Templates", to: "/templates" as const, icon: FileText },
    { label: "Vocabulary", to: "/vocabulary" as const, icon: BookOpen },
    { label: "Context", to: "/context" as const, icon: FileSearch },
    { label: "Codebases", to: "/codebases" as const, icon: FolderGit2 },
    { label: "Workspaces", to: "/workspaces" as const, icon: Layers },
    { label: "Import Docs", to: "/import" as const, icon: FolderUp },
    { label: "Health", to: "/knowledge-health" as const, icon: Activity },
    { label: "Activity", to: "/activity" as const, icon: Clock },
    { label: "Settings", to: "/settings" as const, icon: Settings }
];

export function MobileNav() {
    const [open, setOpen] = useState(false);

    return (
        <Sheet open={open} onOpenChange={setOpen}>
            <SheetTrigger render={<Button variant="ghost" size="sm" className="md:hidden" />}>
                <Menu className="size-5" />
            </SheetTrigger>
            <SheetContent side="left" className="w-64 p-0">
                <div className="p-4">
                    <span className="text-lg font-bold">fubbik</span>
                </div>
                <Separator />
                <nav className="space-y-1 p-2">
                    {primaryItems.map(item => (
                        <Link
                            key={item.to}
                            to={item.to}
                            search={{} as any}
                            onClick={() => setOpen(false)}
                            className="hover:bg-muted flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors"
                        >
                            <item.icon className="size-4" />
                            {item.label}
                        </Link>
                    ))}
                </nav>
                <Separator />
                <div className="px-5 pt-3 pb-1">
                    <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">Manage</span>
                </div>
                <nav className="space-y-1 p-2">
                    {manageItems.map(item => (
                        <Link
                            key={item.to}
                            to={item.to}
                            onClick={() => setOpen(false)}
                            className="hover:bg-muted flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors"
                        >
                            <item.icon className="size-4" />
                            {item.label}
                        </Link>
                    ))}
                </nav>
            </SheetContent>
        </Sheet>
    );
}
