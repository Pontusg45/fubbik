import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Flag } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { useActiveFeatures } from "./use-active-features";

export function FeatureSwitcher() {
    const { activeFeatureIds, toggleFeature, isActive } = useActiveFeatures();

    const { data: features } = useQuery({
        queryKey: ["features"],
        queryFn: async () => unwrapEden(await api.api.features.get({ query: {} })),
        staleTime: 60_000
    });

    if (!features || features.length === 0) return null;

    const activeCount = activeFeatureIds.length;

    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" size="sm" className="relative gap-1.5" />}>
                <Flag className="size-3.5" />
                <span className="hidden sm:inline">Features</span>
                {activeCount > 0 && (
                    <span className="flex size-4 items-center justify-center rounded-full bg-blue-500 text-[10px] font-bold text-white">
                        {activeCount}
                    </span>
                )}
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
                <DropdownMenuLabel>Feature Overlays</DropdownMenuLabel>
                {(features as Array<{ id: string; name: string; color: string | null; priority: number; status: string; deltaCount: number }>).map(f => (
                    <DropdownMenuItem
                        key={f.id}
                        onClick={() => toggleFeature(f.id)}
                        className="flex items-center justify-between"
                    >
                        <span className="flex items-center gap-2">
                            <span
                                className="size-2 rounded-full"
                                style={{ backgroundColor: f.color ?? "#8b5cf6" }}
                            />
                            <span>{f.name}</span>
                            <span className="text-muted-foreground text-xs">({f.deltaCount})</span>
                        </span>
                        <span className={`size-3 rounded-sm border ${isActive(f.id) ? "border-blue-500 bg-blue-500" : "border-muted-foreground"}`} />
                    </DropdownMenuItem>
                ))}
                <DropdownMenuSeparator />
                <DropdownMenuItem render={<Link to="/features" />}>
                    Manage Features
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
