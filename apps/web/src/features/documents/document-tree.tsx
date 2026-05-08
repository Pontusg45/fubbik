import { ChevronDown, ChevronRight, FileText, FolderOpen, Tag } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { filenameFromPath, getStaleness, type FolderNode } from "./document-utils";
import type { DocumentListItem } from "./document-types";

/* ─── Folder Tree Sidebar Node ─── */

export interface FolderTreeNodeProps {
    node: FolderNode;
    depth: number;
    selectedId: string | null;
    onSelect: (id: string) => void;
    defaultOpen: boolean;
}

export function FolderTreeNode({ node, depth, selectedId, onSelect, defaultOpen }: FolderTreeNodeProps) {
    const [open, setOpen] = useState(defaultOpen);
    const hasContent = node.docs.length > 0 || node.children.length > 0;
    if (!hasContent) return null;

    const paddingLeft = depth * 12;

    return (
        <div>
            <button
                type="button"
                onClick={() => setOpen(o => !o)}
                className="text-muted-foreground hover:text-foreground mb-0.5 flex w-full items-center gap-1 px-2 py-1 text-xs font-medium transition-colors"
                style={{ paddingLeft }}
            >
                {open ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                <FolderOpen className="size-3.5" />
                <span className="truncate">{node.name}</span>
            </button>
            {open && (
                <div>
                    {node.docs.map(doc => (
                        <button
                            key={doc.id}
                            onClick={() => onSelect(doc.id)}
                            className={`flex w-full items-center gap-2 rounded-md py-1.5 text-left text-sm transition-colors ${
                                selectedId === doc.id
                                    ? "bg-muted text-foreground font-medium"
                                    : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                            }`}
                            style={{ paddingLeft: paddingLeft + 20 }}
                        >
                            <FileText className="size-3.5 shrink-0" />
                            <span className="min-w-0 flex-1 truncate">{doc.title || filenameFromPath(doc.sourcePath)}</span>
                            <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px] mr-2">
                                {doc.chunkCount}
                            </Badge>
                        </button>
                    ))}
                    {node.children.map(child => (
                        <FolderTreeNode
                            key={child.fullPath}
                            node={child}
                            depth={depth + 1}
                            selectedId={selectedId}
                            onSelect={onSelect}
                            defaultOpen={defaultOpen}
                        />
                    ))}
                </div>
            )}
        </div>
    );
}

/* ─── Index Tree (main content "All Documents" view) ─── */

export interface IndexTreeProps {
    node: FolderNode;
    depth: number;
    onSelect: (id: string) => void;
}

export function IndexTree({ node, depth, onSelect }: IndexTreeProps) {
    return (
        <>
            {depth > 0 && (
                <h3
                    className="text-sm font-semibold text-muted-foreground mb-2 mt-4 flex items-center gap-1.5"
                    style={{ paddingLeft: (depth - 1) * 16 }}
                >
                    <FolderOpen className="size-3.5" />
                    {node.name}
                </h3>
            )}
            <div className="space-y-1" style={{ paddingLeft: depth > 0 ? (depth - 1) * 16 + 20 : 0 }}>
                {node.docs.map(doc => {
                    const staleness = getStaleness(doc);
                    return (
                        <button
                            key={doc.id}
                            onClick={() => onSelect(doc.id)}
                            className="text-foreground hover:text-foreground/80 flex items-center gap-2 text-sm w-full text-left"
                        >
                            <FileText className="size-3.5 text-muted-foreground shrink-0" />
                            <span>{doc.title}</span>
                            {doc.description && (
                                <span className="text-muted-foreground text-xs truncate">— {doc.description}</span>
                            )}
                            <span className={`text-xs ml-auto shrink-0 ${staleness.color}`} title={staleness.tooltip}>
                                {staleness.label}
                            </span>
                        </button>
                    );
                })}
            </div>
            {node.children.map(child => (
                <IndexTree key={child.fullPath} node={child} depth={depth + 1} onSelect={onSelect} />
            ))}
        </>
    );
}

/* ─── Tag Group Sidebar Node ─── */

export interface TagGroupNodeProps {
    name: string;
    docs: DocumentListItem[];
    selectedId: string | null;
    selectedGroup: string | null;
    onSelect: (id: string) => void;
    onGroupSelect: (name: string) => void;
}

export function TagGroupNode({ name, docs, selectedId, selectedGroup, onSelect, onGroupSelect }: TagGroupNodeProps) {
    const [open, setOpen] = useState(true);
    const isGroupSelected = selectedGroup === name;

    return (
        <div>
            <div className="mb-0.5 flex items-center gap-0">
                <button
                    type="button"
                    onClick={() => setOpen(o => !o)}
                    className="text-muted-foreground hover:text-foreground flex items-center py-1 pl-2 transition-colors"
                >
                    {open ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                </button>
                <button
                    type="button"
                    onClick={() => onGroupSelect(name)}
                    className={`flex flex-1 items-center gap-1 rounded-md px-1 py-1 text-xs font-medium transition-colors ${
                        isGroupSelected
                            ? "bg-muted text-foreground"
                            : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                    }`}
                >
                    <Tag className="size-3.5" />
                    <span className="truncate">{name}</span>
                    <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[9px]">
                        {docs.length}
                    </Badge>
                </button>
            </div>
            {open && (
                <div>
                    {docs.map(doc => (
                        <button
                            key={doc.id}
                            onClick={() => onSelect(doc.id)}
                            className={`flex w-full items-center gap-2 rounded-md py-1.5 text-left text-sm transition-colors ${
                                selectedId === doc.id
                                    ? "bg-muted text-foreground font-medium"
                                    : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                            }`}
                            style={{ paddingLeft: 32 }}
                        >
                            <FileText className="size-3.5 shrink-0" />
                            <span className="min-w-0 flex-1 truncate">{doc.title || doc.sourcePath.split("/").pop()}</span>
                            <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px] mr-2">
                                {doc.chunkCount}
                            </Badge>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}
