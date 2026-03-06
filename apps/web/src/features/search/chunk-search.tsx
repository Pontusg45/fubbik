import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Blocks, Search } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogPopup, DialogTrigger } from "@/components/ui/dialog";
import { Kbd } from "@/components/ui/kbd";
import { api } from "@/utils/api";

export function ChunkSearch() {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");

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
        queryKey: ["chunk-search", search],
        queryFn: async () => {
            if (!search.trim()) return null;
            const { data, error } = await api.api.chunks.get({
                query: { search, limit: "10" }
            });
            if (error) return null;
            return data as Exclude<typeof data, { message: string }>;
        },
        enabled: search.length > 1
    });

    const chunks = searchQuery.data?.chunks ?? [];

    return (
        <Dialog open={open} onOpenChange={handleOpenChange}>
            <DialogTrigger
                render={
                    <Button
                        variant="outline"
                        size="sm"
                        className="text-muted-foreground gap-2"
                    />
                }
            >
                <Search className="size-3.5" />
                <span className="hidden sm:inline">Search...</span>
                <Kbd className="hidden sm:inline">&#8984;K</Kbd>
            </DialogTrigger>
            <DialogPopup showCloseButton={false}>
                <div className="p-4 space-y-3">
                    <div className="flex items-center gap-2">
                        <Search className="text-muted-foreground size-4" />
                        <input
                            type="text"
                            value={search}
                            onChange={e => setSearch(e.target.value)}
                            placeholder="Search chunks..."
                            autoFocus
                            className="bg-transparent flex-1 text-sm outline-none placeholder:text-muted-foreground"
                        />
                    </div>
                    <div className="max-h-64 overflow-y-auto space-y-1">
                        {search.length > 1 &&
                            chunks.length === 0 &&
                            !searchQuery.isLoading && (
                                <p className="text-muted-foreground text-center text-sm py-4">
                                    No chunks found.
                                </p>
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
                                className="flex items-center gap-3 rounded-md px-3 py-2 text-sm hover:bg-muted transition-colors"
                            >
                                <Blocks className="text-muted-foreground size-4 shrink-0" />
                                <div className="min-w-0 flex-1">
                                    <p className="truncate font-medium">
                                        {chunk.title}
                                    </p>
                                    <div className="flex items-center gap-1 mt-0.5">
                                        <Badge
                                            variant="secondary"
                                            size="sm"
                                            className="font-mono text-[10px]"
                                        >
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
