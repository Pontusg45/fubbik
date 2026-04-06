import { Link } from "@tanstack/react-router";
import { ArrowDownRight, ArrowUpRight } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { relationColor } from "@/features/chunks/relation-colors";

interface Connection {
    id: string;
    sourceId: string;
    targetId: string;
    relation: string;
    title?: string | null;
    codebaseName?: string | null;
}

interface DependencyTreeProps {
    chunkId: string;
    connections: Connection[];
}

export function DependencyTree({ chunkId, connections }: DependencyTreeProps) {
    if (connections.length === 0) return null;

    const outgoing = connections.filter((c) => c.sourceId === chunkId);
    const incoming = connections.filter((c) => c.targetId === chunkId);

    const groupByRelation = (conns: Connection[]) => {
        const groups: Record<string, Connection[]> = {};
        for (const conn of conns) {
            const key = conn.relation;
            if (!groups[key]) groups[key] = [];
            groups[key].push(conn);
        }
        return groups;
    };

    const outgoingGroups = groupByRelation(outgoing);
    const incomingGroups = groupByRelation(incoming);

    return (
        <div className="grid gap-4 md:grid-cols-2">
            <div>
                <h4 className="text-muted-foreground mb-3 flex items-center gap-1.5 text-xs font-medium">
                    <ArrowDownRight className="size-3.5" />
                    Outgoing ({outgoing.length})
                </h4>
                {outgoing.length === 0 && (
                    <p className="text-muted-foreground text-sm">None</p>
                )}
                <div className="space-y-3">
                    {Object.entries(outgoingGroups).map(([relation, conns]) => (
                        <div key={relation}>
                            <Badge
                                variant="outline"
                                size="sm"
                                className="mb-1.5 text-[10px]"
                                style={{
                                    borderColor: relationColor(relation),
                                    color: relationColor(relation),
                                }}
                            >
                                {relation}
                            </Badge>
                            <ul className="border-muted space-y-1 border-l-2 pl-3">
                                {conns.map((conn) => (
                                    <li key={conn.id} className="text-sm">
                                        <Link
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: conn.targetId }}
                                            className="text-foreground hover:text-primary underline-offset-2 hover:underline"
                                        >
                                            {conn.title ?? conn.targetId}
                                        </Link>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    ))}
                </div>
            </div>

            <div>
                <h4 className="text-muted-foreground mb-3 flex items-center gap-1.5 text-xs font-medium">
                    <ArrowUpRight className="size-3.5" />
                    Incoming ({incoming.length})
                </h4>
                {incoming.length === 0 && (
                    <p className="text-muted-foreground text-sm">None</p>
                )}
                <div className="space-y-3">
                    {Object.entries(incomingGroups).map(([relation, conns]) => (
                        <div key={relation}>
                            <Badge
                                variant="outline"
                                size="sm"
                                className="mb-1.5 text-[10px]"
                                style={{
                                    borderColor: relationColor(relation),
                                    color: relationColor(relation),
                                }}
                            >
                                {relation}
                            </Badge>
                            <ul className="border-muted space-y-1 border-l-2 pl-3">
                                {conns.map((conn) => (
                                    <li key={conn.id} className="text-sm">
                                        <Link
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: conn.sourceId }}
                                            className="text-foreground hover:text-primary underline-offset-2 hover:underline"
                                        >
                                            {conn.title ?? conn.sourceId}
                                        </Link>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}
