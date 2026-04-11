import { Link } from "@tanstack/react-router";
import { AlertTriangle, Clock, Link2Off, Network, ScrollText } from "lucide-react";

const COLLECTIONS = [
    { label: "Recently updated", icon: Clock, q: "updated:7d" },
    { label: "Needs review", icon: AlertTriangle, q: "review:draft" },
    { label: "Well-connected", icon: Network, q: "connections:5+" },
    { label: "Deep dives", icon: ScrollText, q: "type:document" },
    { label: "Orphans", icon: Link2Off, q: "connections:0+" },
];

export function SmartCollections() {
    return (
        <div className="rounded-lg border">
            <div className="border-b px-4 py-3">
                <h2 className="text-sm font-semibold">Smart collections</h2>
            </div>
            <div className="divide-y">
                {COLLECTIONS.map(col => (
                    <Link
                        key={col.label}
                        to="/search"
                        search={{ q: col.q } as any}
                        className="flex items-center gap-3 px-4 py-3 hover:bg-muted/50 transition-colors"
                    >
                        <col.icon className="size-4 text-muted-foreground" />
                        <span className="text-sm">{col.label}</span>
                        <code className="ml-auto text-[10px] text-muted-foreground font-mono">{col.q}</code>
                    </Link>
                ))}
            </div>
        </div>
    );
}
