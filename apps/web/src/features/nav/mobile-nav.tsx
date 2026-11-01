import { Link } from "@tanstack/react-router";
import { ChevronDown, Menu } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet";
import { MANAGE_NAV, PRIMARY_NAV } from "@/features/nav/navigation-items";
import { filterNavItems, useCapabilities } from "@/features/nav/use-capabilities";

export function MobileNav() {
    const [open, setOpen] = useState(false);
    const [manageOpen, setManageOpen] = useState(false);
    const capabilities = useCapabilities();

    return (
        <Sheet open={open} onOpenChange={setOpen}>
            <SheetTrigger render={<Button variant="ghost" size="sm" className="md:hidden" aria-label="Open navigation menu" />}>
                <Menu className="size-5" />
            </SheetTrigger>
            <SheetContent side="left" className="w-64 overflow-y-auto p-0">
                <div className="p-4">
                    <span className="text-lg font-bold">fubbik</span>
                </div>
                <Separator />
                <nav className="space-y-1 p-2">
                    {filterNavItems(PRIMARY_NAV, capabilities).map(item => (
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
                <button
                    type="button"
                    aria-expanded={manageOpen}
                    onClick={() => setManageOpen(!manageOpen)}
                    className="flex w-full items-center justify-between px-5 pt-3 pb-1"
                >
                    <span className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Manage</span>
                    <ChevronDown className={`text-muted-foreground size-3.5 transition-transform ${manageOpen ? "rotate-180" : ""}`} />
                </button>
                {manageOpen && (
                    <nav className="space-y-1 p-2">
                        {filterNavItems(MANAGE_NAV, capabilities).map(item => (
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
                )}
            </SheetContent>
        </Sheet>
    );
}
