import { Link } from "@tanstack/react-router";
import { MoreHorizontal, Pencil, Pin, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";

interface ChunkRowActionsProps {
    chunkId: string;
    isPinned: boolean;
    onTogglePin: () => void;
    onDelete: () => void;
}

export function ChunkRowActions({ chunkId, isPinned, onTogglePin, onDelete }: ChunkRowActionsProps) {
    return (
        <DropdownMenu>
            <DropdownMenuTrigger
                render={
                    <Button variant="ghost" size="sm" className="text-muted-foreground hover:text-foreground size-7 p-0">
                        <MoreHorizontal className="size-4" />
                    </Button>
                }
            />
            <DropdownMenuContent align="end">
                <DropdownMenuItem render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}>
                    <Pencil className="size-3.5" />
                    Edit
                </DropdownMenuItem>
                <DropdownMenuItem onClick={onTogglePin}>
                    <Pin className={`size-3.5 ${isPinned ? "fill-current" : ""}`} />
                    {isPinned ? "Unpin" : "Pin"}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem variant="destructive" onClick={onDelete}>
                    <Trash2 className="size-3.5" />
                    Delete
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
