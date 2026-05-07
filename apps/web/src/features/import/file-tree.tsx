
import { ChevronDown, ChevronRight, File, Folder } from "lucide-react";
import { useState, useCallback } from "react";

import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { type TreeNode, INDEX_FILE_NAMES } from "./types";

// ---------------------------------------------------------------------------
// buildTree — converts flat file paths into a nested tree structure
// ---------------------------------------------------------------------------

export function buildTree(paths: string[]): TreeNode[] {
    const root: TreeNode[] = [];

    for (const filePath of paths) {
        const parts = filePath.split("/");
        let current = root;

        for (let i = 0; i < parts.length; i++) {
            const part = parts[i] as string;
            const isLastPart = i === parts.length - 1;
            const currentPath = parts.slice(0, i + 1).join("/");

            if (isLastPart) {
                // It's a file
                const fileName = part.toLowerCase();
                const isIndex = INDEX_FILE_NAMES.has(fileName);
                current.push({
                    name: part,
                    path: filePath,
                    isDir: false,
                    isIndex,
                    children: []
                });
            } else {
                // It's a directory segment
                let existing = current.find(n => n.isDir && n.name === part);
                if (!existing) {
                    const newDir: TreeNode = {
                        name: part,
                        path: currentPath,
                        isDir: true,
                        isIndex: false,
                        children: []
                    };
                    current.push(newDir);
                    existing = newDir;
                }
                current = existing.children;
            }
        }
    }

    return root;
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

export function countFiles(nodes: TreeNode[]): number {
    let count = 0;
    for (const node of nodes) {
        if (!node.isDir) {
            count++;
        } else {
            count += countFiles(node.children);
        }
    }
    return count;
}

type CheckState = "checked" | "unchecked" | "indeterminate";

export function getCheckState(node: TreeNode, selected: Set<string>): CheckState {
    if (!node.isDir) {
        return selected.has(node.path) ? "checked" : "unchecked";
    }

    const files = getAllFiles(node.children);
    if (files.length === 0) return "unchecked";

    const selectedCount = files.filter(f => selected.has(f)).length;
    if (selectedCount === 0) return "unchecked";
    if (selectedCount === files.length) return "checked";
    return "indeterminate";
}

export function getAllFiles(nodes: TreeNode[]): string[] {
    const files: string[] = [];
    for (const node of nodes) {
        if (!node.isDir) {
            files.push(node.path);
        } else {
            files.push(...getAllFiles(node.children));
        }
    }
    return files;
}

export function findIndexFile(siblings: TreeNode[]): TreeNode | undefined {
    return siblings.find(n => !n.isDir && n.isIndex);
}

export function filterTree(nodes: TreeNode[], filter: string): TreeNode[] {
    if (!filter) return nodes;
    const lowerFilter = filter.toLowerCase();

    return nodes.reduce<TreeNode[]>((acc, node) => {
        if (!node.isDir) {
            if (node.name.toLowerCase().includes(lowerFilter)) {
                acc.push(node);
            }
        } else {
            const filteredChildren = filterTree(node.children, filter);
            if (filteredChildren.length > 0) {
                acc.push({ ...node, children: filteredChildren });
            }
        }
        return acc;
    }, []);
}

// ---------------------------------------------------------------------------
// FileTree component
// ---------------------------------------------------------------------------

interface FileTreeProps {
    nodes: TreeNode[];
    selected: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    mode?: "checkbox" | "navigate";
    activePath?: string;
    onFileClick?: (path: string) => void;
    templatePaths?: Set<string>;
    filter?: string;
}

export function FileTree({
    nodes,
    selected,
    onSelectionChange,
    mode = "checkbox",
    activePath,
    onFileClick,
    templatePaths,
    filter
}: FileTreeProps) {
    const totalFiles = countFiles(nodes);
    const defaultExpanded = totalFiles < 30;

    const displayNodes = filter ? filterTree(nodes, filter) : nodes;

    return (
        <div className="select-none text-sm">
            {displayNodes.map(node => (
                <FileTreeNode
                    key={node.path}
                    node={node}
                    selected={selected}
                    onSelectionChange={onSelectionChange}
                    mode={mode}
                    activePath={activePath}
                    onFileClick={onFileClick}
                    templatePaths={templatePaths}
                    depth={0}
                    defaultExpanded={defaultExpanded}
                    siblings={displayNodes}
                />
            ))}
        </div>
    );
}

// ---------------------------------------------------------------------------
// FileTreeNode component — recursive node renderer
// ---------------------------------------------------------------------------

interface FileTreeNodeProps {
    node: TreeNode;
    selected: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    mode: "checkbox" | "navigate";
    activePath?: string;
    onFileClick?: (path: string) => void;
    templatePaths?: Set<string>;
    depth: number;
    defaultExpanded: boolean;
    siblings: TreeNode[];
}

function FileTreeNode({
    node,
    selected,
    onSelectionChange,
    mode,
    activePath,
    onFileClick,
    templatePaths,
    depth,
    defaultExpanded,
    siblings
}: FileTreeNodeProps) {
    const [expanded, setExpanded] = useState(defaultExpanded);

    const handleToggleExpand = useCallback((e: React.MouseEvent) => {
        e.stopPropagation();
        setExpanded(prev => !prev);
    }, []);

    const indentStyle = { paddingLeft: `${depth * 16 + 4}px` };

    if (node.isDir) {
        return (
            <FolderNode
                node={node}
                selected={selected}
                onSelectionChange={onSelectionChange}
                mode={mode}
                activePath={activePath}
                onFileClick={onFileClick}
                templatePaths={templatePaths}
                depth={depth}
                defaultExpanded={defaultExpanded}
                expanded={expanded}
                indentStyle={indentStyle}
                onToggleExpand={handleToggleExpand}
            />
        );
    }

    return (
        <FileNode
            node={node}
            selected={selected}
            onSelectionChange={onSelectionChange}
            mode={mode}
            activePath={activePath}
            onFileClick={onFileClick}
            depth={depth}
            indentStyle={indentStyle}
            siblings={siblings}
        />
    );
}

// ---------------------------------------------------------------------------
// FolderNode
// ---------------------------------------------------------------------------

interface FolderNodeProps {
    node: TreeNode;
    selected: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    mode: "checkbox" | "navigate";
    activePath?: string;
    onFileClick?: (path: string) => void;
    templatePaths?: Set<string>;
    depth: number;
    defaultExpanded: boolean;
    expanded: boolean;
    indentStyle: React.CSSProperties;
    onToggleExpand: (e: React.MouseEvent) => void;
}

function FolderNode({
    node,
    selected,
    onSelectionChange,
    mode,
    activePath,
    onFileClick,
    templatePaths,
    depth,
    defaultExpanded,
    expanded,
    indentStyle,
    onToggleExpand
}: FolderNodeProps) {
    const allFiles = getAllFiles(node.children);
    const totalFileCount = allFiles.length;
    const selectedFileCount = allFiles.filter(f => selected.has(f)).length;
    const checkState = getCheckState(node, selected);

    const templateCount =
        mode === "navigate" && templatePaths
            ? allFiles.filter(f => templatePaths.has(f)).length
            : 0;

    const handleFolderCheckChange = useCallback(() => {
        const files = getAllFiles(node.children);
        const next = new Set(selected);
        if (checkState === "checked") {
            for (const f of files) next.delete(f);
        } else {
            for (const f of files) next.add(f);
        }
        onSelectionChange(next);
    }, [node.children, selected, checkState, onSelectionChange]);

    return (
        <div>
            <div
                className="flex cursor-pointer items-center gap-1.5 rounded px-1 py-0.5 hover:bg-muted/50"
                style={indentStyle}
                onClick={onToggleExpand}
            >
                {/* Expand/collapse chevron */}
                <span className="shrink-0 text-muted-foreground">
                    {expanded ? (
                        <ChevronDown className="size-3.5" />
                    ) : (
                        <ChevronRight className="size-3.5" />
                    )}
                </span>

                {/* Checkbox in checkbox mode */}
                {mode === "checkbox" && (
                    <Checkbox
                        checked={checkState === "checked"}
                        indeterminate={checkState === "indeterminate"}
                        onCheckedChange={handleFolderCheckChange}
                        onClick={e => e.stopPropagation()}
                    />
                )}

                <Folder className="size-3.5 shrink-0 text-muted-foreground" />

                <span className="truncate font-medium text-foreground">{node.name}</span>

                {/* File count badge */}
                <Badge variant="outline" size="sm" className="ml-auto shrink-0">
                    {totalFileCount}
                </Badge>

                {/* Partial selection badge in checkbox mode */}
                {mode === "checkbox" && checkState === "indeterminate" && (
                    <Badge variant="secondary" size="sm" className="shrink-0">
                        {selectedFileCount} of {totalFileCount}
                    </Badge>
                )}

                {/* Template count badge in navigate mode */}
                {mode === "navigate" && templateCount > 0 && (
                    <Badge variant="success" size="sm" className="shrink-0">
                        {templateCount} matched
                    </Badge>
                )}
            </div>

            {expanded && (
                <div>
                    {node.children.map(child => (
                        <FileTreeNode
                            key={child.path}
                            node={child}
                            selected={selected}
                            onSelectionChange={onSelectionChange}
                            mode={mode}
                            activePath={activePath}
                            onFileClick={onFileClick}
                            templatePaths={templatePaths}
                            depth={depth + 1}
                            defaultExpanded={defaultExpanded}
                            siblings={node.children}
                        />
                    ))}
                </div>
            )}
        </div>
    );
}

// ---------------------------------------------------------------------------
// FileNode
// ---------------------------------------------------------------------------

interface FileNodeProps {
    node: TreeNode;
    selected: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    mode: "checkbox" | "navigate";
    activePath?: string;
    onFileClick?: (path: string) => void;
    depth: number;
    indentStyle: React.CSSProperties;
    siblings: TreeNode[];
}

function FileNode({
    node,
    selected,
    onSelectionChange,
    mode,
    activePath,
    onFileClick,
    indentStyle,
    siblings
}: FileNodeProps) {
    const isChecked = selected.has(node.path);
    const isActive = activePath === node.path;

    // Connection hint: non-index files that share a directory with an index file
    const indexSibling = !node.isIndex ? findIndexFile(siblings) : undefined;

    const handleCheckChange = useCallback(() => {
        const next = new Set(selected);
        if (isChecked) {
            next.delete(node.path);
        } else {
            next.add(node.path);
        }
        onSelectionChange(next);
    }, [selected, isChecked, node.path, onSelectionChange]);

    const handleClick = useCallback(() => {
        if (mode === "checkbox") {
            handleCheckChange();
        } else {
            onFileClick?.(node.path);
        }
    }, [mode, handleCheckChange, onFileClick, node.path]);

    return (
        <div
            className={`flex cursor-pointer items-center gap-1.5 rounded px-1 py-0.5 hover:bg-muted/50 ${
                isActive ? "bg-accent/60" : ""
            }`}
            style={indentStyle}
            onClick={handleClick}
        >
            {/* Spacer to align with folder chevron */}
            <span className="size-3.5 shrink-0" />

            {/* Checkbox in checkbox mode */}
            {mode === "checkbox" && (
                <Checkbox
                    checked={isChecked}
                    onCheckedChange={handleCheckChange}
                    onClick={e => e.stopPropagation()}
                />
            )}

            <File className="size-3.5 shrink-0 text-muted-foreground" />

            <span className="truncate text-foreground">{node.name}</span>

            {/* Index badge */}
            {node.isIndex && (
                <Badge variant="warning" size="sm" className="shrink-0">
                    index
                </Badge>
            )}

            {/* Connection hint in checkbox mode */}
            {mode === "checkbox" && !node.isIndex && indexSibling && (
                <span className="ml-1 shrink-0 truncate text-xs text-muted-foreground">
                    → part_of {indexSibling.name}
                </span>
            )}
        </div>
    );
}
