import { GitMerge, MoreHorizontal, Palette, Pencil, Trash2 } from "lucide-react";
import { Link } from "@tanstack/react-router";

import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuSub,
    DropdownMenuSubContent,
    DropdownMenuSubTrigger,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

import type { Tag, TagType } from "./tag-types";

export interface TagPillProps {
    tag: Tag;
    tagTypes: TagType[];
    isRenaming: boolean;
    renameValue: string;
    setRenameValue: (v: string) => void;
    onStartRename: () => void;
    onCommitRename: () => void;
    onCancelRename: () => void;
    onAssignType: (tagTypeId: string | null) => void;
    onMerge: () => void;
    onDelete: () => void;
}

export function TagPill({
    tag,
    tagTypes,
    isRenaming,
    renameValue,
    setRenameValue,
    onStartRename,
    onCommitRename,
    onCancelRename,
    onAssignType,
    onMerge,
    onDelete,
}: TagPillProps) {
    return (
        <div className="group flex items-center gap-1.5 rounded-lg border px-3 py-1.5 transition-colors hover:bg-muted/50">
            {tag.tagTypeColor && (
                <div
                    className="size-2 rounded-full shrink-0"
                    style={{ backgroundColor: tag.tagTypeColor }}
                />
            )}
            {isRenaming ? (
                <input
                    value={renameValue}
                    onChange={e => setRenameValue(e.target.value)}
                    onBlur={onCommitRename}
                    onKeyDown={e => {
                        if (e.key === "Enter") { e.preventDefault(); onCommitRename(); }
                        if (e.key === "Escape") { e.preventDefault(); onCancelRename(); }
                    }}
                    autoFocus
                    className="bg-background w-24 rounded border px-1 text-sm outline-none focus:ring-1 focus:ring-ring"
                />
            ) : (
                <Link
                    to="/chunks"
                    search={{ tags: tag.name }}
                    className="text-sm hover:underline"
                >
                    {tag.name}
                </Link>
            )}
            {tag.chunkCount > 0 && (
                <span className="text-muted-foreground ml-0.5 text-xs tabular-nums">{tag.chunkCount}</span>
            )}
            {!isRenaming && (
                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <button
                                aria-label={`Tag actions for ${tag.name}`}
                                className="text-muted-foreground/0 group-hover:text-muted-foreground hover:text-foreground pointer-coarse:text-muted-foreground ml-1 transition-colors"
                            >
                                <MoreHorizontal className="size-3.5" />
                            </button>
                        }
                    />
                    <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={onStartRename}>
                            <Pencil className="size-3.5" />
                            Rename
                        </DropdownMenuItem>
                        <DropdownMenuSub>
                            <DropdownMenuSubTrigger>
                                <Palette className="size-3.5" />
                                Assign type
                            </DropdownMenuSubTrigger>
                            <DropdownMenuSubContent>
                                <DropdownMenuItem onClick={() => onAssignType(null)}>
                                    <div className="border-muted-foreground/50 size-2.5 rounded-full border" />
                                    None
                                </DropdownMenuItem>
                                {tagTypes.length > 0 && <DropdownMenuSeparator />}
                                {tagTypes.map(tt => (
                                    <DropdownMenuItem key={tt.id} onClick={() => onAssignType(tt.id)}>
                                        <div className="size-2.5 rounded-full" style={{ backgroundColor: tt.color }} />
                                        {tt.name}
                                    </DropdownMenuItem>
                                ))}
                            </DropdownMenuSubContent>
                        </DropdownMenuSub>
                        <DropdownMenuItem onClick={onMerge}>
                            <GitMerge className="size-3.5" />
                            Merge into…
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem onClick={onDelete} className="text-destructive">
                            <Trash2 className="size-3.5" />
                            Delete
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            )}
        </div>
    );
}
