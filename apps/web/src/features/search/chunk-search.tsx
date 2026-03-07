import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Blocks, Search } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogPopup, DialogTrigger } from "@/components/ui/dialog";
import { Kbd } from "@/components/ui/kbd";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function ChunkSearch() {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");
    const debouncedSearch = useDebouncedValue(search, 200);

    // Global Cmd+K / Ctrl+K shortcut
    useEffect(() => {
        const down = (e: KeyboardEvent) => {
            if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                setOpen(prev => !prev);
            }
        };
        document.addEventListener("keydown", down);
        return () => document.removeEventListener("keydown", down);
    }, []);

    // Reset search when dialog closes
    const handleOpenChange = (nextOpen: boolean) => {
        setOpen(nextOpen);
        if (!nextOpen) setSearch("");
    };

    const searchQuery = useQuery({
        queryKey: ["chunk-search", debouncedSearch],
        queryFn: async () => {
            if (!debouncedSearch.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: { search: debouncedSearch, limit: "10" }
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: debouncedSearch.length > 1
    });

    const chunks = searchQuery.data?.chunks ?? [];

    return (
        <Dialog open={open} onOpenChange={handleOpenChange}>
            <DialogTrigger render={<Button variant="outline" size="sm" className="text-muted-foreground gap-2" />}>
                <Search className="size-3.5" />
                <span className="hidden sm:inline">Search...</span>
                <Kbd className="hidden sm:inline">&#8984;K</Kbd>
            </DialogTrigger>
            <DialogPopup showCloseButton={false}>
                <div className="space-y-3 p-4">
                    <div className="flex items-center gap-2">
                        <Search className="text-muted-foreground size-4" />
                        <input
                            type="text"
                            value={search}
                            onChange={e => setSearch(e.target.value)}
                            placeholder="Search chunks..."
                            autoFocus
                            className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm outline-none"
                        />
                    </div>
                    <div className="max-h-64 space-y-1 overflow-y-auto">
                        {search.length > 1 && chunks.length === 0 && !searchQuery.isLoading && (
                            <p className="text-muted-foreground py-4 text-center text-sm">No chunks found.</p>
                        )}
                        {chunks.map(chunk => (
                            <Link
                                key={chunk.id}
                                to="/chunks/$chunkId"
                                params={{ chunkId: chunk.id }}
                                onClick={() => {
                                    setOpen(false);
                                    setSearch("");
                                }}
                                className="hover:bg-muted flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors"
                            >
                                <Blocks className="text-muted-foreground size-4 shrink-0" />
                                <div className="min-w-0 flex-1">
                                    <p className="truncate font-medium">{chunk.title}</p>
                                    <div className="mt-0.5 flex items-center gap-1">
                                        <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                            {chunk.type}
                                        </Badge>
                                    </div>
                                </div>
                            </Link>
                        ))}
                    </div>
                </div>
            </DialogPopup>
        </Dialog>
    );
}
