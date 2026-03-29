import { Link } from "@tanstack/react-router";
import { Clock, FileText } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useRecentlyViewed } from "@/hooks/use-recently-viewed";

function timeAgo(dateStr: string): string {
    const seconds = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

export function RecentlyViewed() {
    const { items } = useRecentlyViewed();

    return (
        <Popover>
            <PopoverTrigger className="text-muted-foreground hover:text-foreground relative rounded-md p-1.5 transition-colors">
                <Clock className="size-4" />
                {items.length > 0 && (
                    <span className="bg-primary absolute -top-0.5 -right-0.5 size-2 rounded-full" />
                )}
            </PopoverTrigger>
            <PopoverContent side="bottom" align="end" className="w-72">
                <p className="mb-3 text-sm font-semibold">Recently Viewed</p>
                {items.length === 0 ? (
                    <p className="text-muted-foreground py-4 text-center text-xs">
                        No recently viewed chunks
                    </p>
                ) : (
                    <div className="-mx-4 space-y-0.5">
                        {items.map(item => (
                            <Link
                                key={item.id}
                                to="/chunks/$chunkId"
                                params={{ chunkId: item.id }}
                                className="hover:bg-muted flex items-center gap-2 rounded-md px-4 py-2 transition-colors"
                            >
                                <FileText className="text-muted-foreground size-3.5 shrink-0" />
                                <div className="min-w-0 flex-1">
                                    <p className="truncate text-sm font-medium">{item.title}</p>
                                    <div className="flex items-center gap-2">
                                        <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                            {item.type}
                                        </Badge>
                                        <span className="text-muted-foreground text-[10px]">
                                            {timeAgo(item.viewedAt)}
                                        </span>
                                    </div>
                                </div>
                            </Link>
                        ))}
                    </div>
                )}
            </PopoverContent>
        </Popover>
    );
}
