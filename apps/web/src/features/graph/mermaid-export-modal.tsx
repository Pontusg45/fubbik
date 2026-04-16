import { AlertTriangle, Check, Copy, Download } from "lucide-react";
import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogPopup, DialogTitle } from "@/components/ui/dialog";
import { buildMermaidFromGraph, type MermaidExportResult } from "./mermaid-export";

interface MermaidExportModalProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    nodes: Parameters<typeof buildMermaidFromGraph>[0];
    edges: Parameters<typeof buildMermaidFromGraph>[1];
}

export function MermaidExportModal({ open, onOpenChange, nodes, edges }: MermaidExportModalProps) {
    const [copied, setCopied] = useState(false);
    const [direction, setDirection] = useState<"LR" | "TB">("LR");

    const result: MermaidExportResult = useMemo(
        () => buildMermaidFromGraph(nodes, edges, { direction }),
        [nodes, edges, direction]
    );

    async function handleCopy() {
        try {
            await navigator.clipboard.writeText(result.text);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
        } catch {
            // clipboard blocked — fall through silently
        }
    }

    function handleDownload() {
        const blob = new Blob([result.text], { type: "text/plain;charset=utf-8" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = "graph.mmd";
        link.click();
        URL.revokeObjectURL(url);
    }

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogPopup className="max-w-2xl">
                <DialogHeader>
                    <DialogTitle>Export as Mermaid</DialogTitle>
                    <p className="text-muted-foreground text-sm">
                        {result.nodeCount} node{result.nodeCount === 1 ? "" : "s"} · {result.edgeCount} edge
                        {result.edgeCount === 1 ? "" : "s"}
                    </p>
                </DialogHeader>

                <div className="space-y-3 px-6 pb-6">
                    <div className="flex items-center gap-3 text-xs">
                        <label className="text-muted-foreground">Direction</label>
                        <div className="flex gap-1">
                            <button
                                onClick={() => setDirection("LR")}
                                className={`rounded border px-2 py-0.5 ${
                                    direction === "LR"
                                        ? "bg-primary text-primary-foreground"
                                        : "text-muted-foreground hover:text-foreground"
                                }`}
                            >
                                Left → Right
                            </button>
                            <button
                                onClick={() => setDirection("TB")}
                                className={`rounded border px-2 py-0.5 ${
                                    direction === "TB"
                                        ? "bg-primary text-primary-foreground"
                                        : "text-muted-foreground hover:text-foreground"
                                }`}
                            >
                                Top → Bottom
                            </button>
                        </div>
                    </div>

                    {result.truncated && (
                        <div className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-2.5 text-xs text-amber-600 dark:text-amber-400">
                            <AlertTriangle className="size-3.5 shrink-0 mt-0.5" />
                            <span>
                                Graph truncated to {result.nodeCount} nodes. Mermaid becomes unreadable past ~100 nodes —
                                filter or focus the view before exporting for best results.
                            </span>
                        </div>
                    )}

                    <textarea
                        readOnly
                        value={result.text}
                        className="font-mono bg-muted/50 text-xs w-full h-64 resize-none rounded-md border p-3"
                        onFocus={e => e.currentTarget.select()}
                    />

                    <div className="flex items-center justify-end gap-2">
                        <Button variant="outline" size="sm" onClick={handleCopy}>
                            {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
                            {copied ? "Copied" : "Copy"}
                        </Button>
                        <Button variant="outline" size="sm" onClick={handleDownload}>
                            <Download className="size-3.5" />
                            Download .mmd
                        </Button>
                    </div>
                </div>
            </DialogPopup>
        </Dialog>
    );
}
