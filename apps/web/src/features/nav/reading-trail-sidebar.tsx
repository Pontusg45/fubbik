import { Link } from "@tanstack/react-router";
import { ChevronRight, Footprints, X } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { useReadingTrail } from "@/hooks/use-reading-trail";

export function ReadingTrailSidebar() {
    const [collapsed, setCollapsed] = useState(true);
    const { items, clear } = useReadingTrail();

    if (items.length === 0) return null;

    return (
        <div className="fixed right-4 bottom-4 z-40 print:hidden">
            {collapsed ? (
                <button
                    type="button"
                    onClick={() => setCollapsed(false)}
                    className="bg-card hover:bg-muted/60 flex items-center gap-2 rounded-full border px-3 py-2 text-xs shadow-lg transition-colors"
                    aria-label="Show reading trail"
                >
                    <Footprints className="size-3.5" />
                    <span>Trail ({items.length})</span>
                </button>
            ) : (
                <div className="bg-card w-64 rounded-lg border shadow-xl">
                    <div className="flex items-center justify-between border-b px-3 py-2">
                        <div className="flex items-center gap-1.5 text-xs font-semibold">
                            <Footprints className="size-3.5" />
                            Reading trail
                        </div>
                        <div className="flex items-center gap-1">
                            <button type="button" onClick={clear} className="text-muted-foreground hover:text-foreground text-[10px]">
                                Clear
                            </button>
                            <button
                                type="button"
                                onClick={() => setCollapsed(true)}
                                className="text-muted-foreground hover:text-foreground"
                                aria-label="Collapse trail"
                            >
                                <X className="size-3" />
                            </button>
                        </div>
                    </div>
                    <div className="max-h-64 overflow-y-auto p-1">
                        {items.map((item, i) => (
                            <Link
                                key={`${item.id}-${i}`}
                                to="/chunks/$chunkId"
                                params={{ chunkId: item.id }}
                                className="hover:bg-muted flex items-center gap-2 rounded-md px-2 py-1.5 transition-colors"
                            >
                                <ChevronRight className="text-muted-foreground/40 size-3 shrink-0" />
                                <span className="truncate text-xs">{item.title}</span>
                                <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[8px]">
                                    {item.type}
                                </Badge>
                            </Link>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
