import { Link } from "@tanstack/react-router";
import { Edit, ExternalLink, Maximize, Plus, RotateCcw, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";

interface GraphContextMenuProps {
    x: number;
    y: number;
    nodeId?: string | null;
    onClose: () => void;
    onFitView: () => void;
    onResetLayout?: () => void;
    onDelete?: (nodeId: string) => void;
}

export function GraphContextMenu({
    x,
    y,
    nodeId,
    onClose,
    onFitView,
    onResetLayout,
    onDelete,
}: GraphContextMenuProps) {
    const menuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        function handleClickOutside(e: MouseEvent) {
            if (menuRef.current && !menuRef.current.contains(e.target as HTMLElement)) {
                onClose();
            }
        }
        function handleEscape(e: KeyboardEvent) {
            if (e.key === "Escape") onClose();
        }
        document.addEventListener("mousedown", handleClickOutside);
        document.addEventListener("keydown", handleEscape);
        return () => {
            document.removeEventListener("mousedown", handleClickOutside);
            document.removeEventListener("keydown", handleEscape);
        };
    }, [onClose]);

    return (
        <div
            ref={menuRef}
            className="bg-popover text-popover-foreground fixed z-50 min-w-[160px] rounded-md border p-1 shadow-md"
            style={{ left: x, top: y }}
        >
            {nodeId ? (
                <>
                    <Link
                        to="/chunks/$chunkId/edit"
                        params={{ chunkId: nodeId }}
                        className="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors"
                        onClick={onClose}
                    >
                        <Edit className="size-3.5" />
                        Edit
                    </Link>
                    <Link
                        to="/chunks/$chunkId"
                        params={{ chunkId: nodeId }}
                        className="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors"
                        onClick={onClose}
                    >
                        <ExternalLink className="size-3.5" />
                        Open detail
                    </Link>
                    {onDelete && (
                        <>
                            <div className="bg-border my-1 h-px" />
                            <button
                                type="button"
                                className="hover:bg-destructive hover:text-destructive-foreground flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm text-red-500 transition-colors"
                                onClick={() => {
                                    onDelete(nodeId);
                                    onClose();
                                }}
                            >
                                <Trash2 className="size-3.5" />
                                Delete
                            </button>
                        </>
                    )}
                </>
            ) : (
                <>
                    <Link
                        to="/chunks/new"
                        className="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors"
                        onClick={onClose}
                    >
                        <Plus className="size-3.5" />
                        Create new chunk
                    </Link>
                    <div className="bg-border my-1 h-px" />
                    <button
                        type="button"
                        className="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors"
                        onClick={() => {
                            onFitView();
                            onClose();
                        }}
                    >
                        <Maximize className="size-3.5" />
                        Fit view
                    </button>
                    {onResetLayout && (
                        <button
                            type="button"
                            className="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors"
                            onClick={() => {
                                onResetLayout();
                                onClose();
                            }}
                        >
                            <RotateCcw className="size-3.5" />
                            Reset layout
                        </button>
                    )}
                </>
            )}
        </div>
    );
}
