import { Link } from "@tanstack/react-router";
import { Blocks, LayoutDashboard, Menu, Network, Tags } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet";

const navItems = [
    { label: "Dashboard", to: "/dashboard" as const, icon: LayoutDashboard },
    { label: "Chunks", to: "/chunks" as const, icon: Blocks },
    { label: "Graph", to: "/graph" as const, icon: Network },
    { label: "Tags", to: "/tags" as const, icon: Tags },
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
                    {navItems.map(item => (
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
