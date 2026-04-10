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
        <div className="fixed bottom-4 right-4 z-40 print:hidden">
            {collapsed ? (
                <button
                    type="button"
                    onClick={() => setCollapsed(false)}
                    className="flex items-center gap-2 rounded-full border bg-card shadow-lg hover:bg-muted/60 transition-colors px-3 py-2 text-xs"
                    aria-label="Show reading trail"
                >
                    <Footprints className="size-3.5" />
                    <span>Trail ({items.length})</span>
                </button>
            ) : (
                <div className="w-64 rounded-lg border bg-card shadow-xl">
                    <div className="flex items-center justify-between border-b px-3 py-2">
                        <div className="flex items-center gap-1.5 text-xs font-semibold">
                            <Footprints className="size-3.5" />
                            Reading trail
                        </div>
                        <div className="flex items-center gap-1">
                            <button
                                type="button"
                                onClick={clear}
                                className="text-[10px] text-muted-foreground hover:text-foreground"
                            >
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
                                className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted transition-colors"
                            >
                                <ChevronRight className="size-3 shrink-0 text-muted-foreground/40" />
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
