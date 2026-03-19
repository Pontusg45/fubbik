import { Link2, MousePointer, Move, Search } from "lucide-react";

import { Button } from "@/components/ui/button";

interface GraphWelcomeProps {
    onDismiss: () => void;
}

export function GraphWelcome({ onDismiss }: GraphWelcomeProps) {
    return (
        <div className="bg-background/80 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm">
            <div className="bg-background max-w-md rounded-lg border p-6 shadow-lg">
                <h3 className="mb-1 text-base font-semibold">Knowledge Graph</h3>
                <p className="text-muted-foreground mb-4 text-sm">Navigate your knowledge visually.</p>
                <div className="space-y-3 text-sm">
                    <div className="flex items-center gap-3">
                        <MousePointer className="text-muted-foreground size-4 shrink-0" />
                        <span>
                            <strong>Click</strong> a node to see details. <strong>Double-click</strong> to open.
                        </span>
                    </div>
                    <div className="flex items-center gap-3">
                        <Move className="text-muted-foreground size-4 shrink-0" />
                        <span>
                            <strong>Drag</strong> nodes to rearrange. Scroll to zoom.
                        </span>
                    </div>
                    <div className="flex items-center gap-3">
                        <Link2 className="text-muted-foreground size-4 shrink-0" />
                        <span>
                            <strong>Shift+click</strong> two nodes to find the path between them.
                        </span>
                    </div>
                    <div className="flex items-center gap-3">
                        <Search className="text-muted-foreground size-4 shrink-0" />
                        <span>
                            Press{" "}
                            <kbd className="bg-muted rounded border px-1 text-xs">?</kbd> for
                            all shortcuts.
                        </span>
                    </div>
                </div>
                <Button onClick={onDismiss} className="mt-5 w-full" size="sm">
                    Got it
                </Button>
            </div>
        </div>
    );
}
