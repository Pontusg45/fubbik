import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { LinkIcon } from "lucide-react";
import { useMemo } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { PageLoading } from "@/components/ui/page";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const KNOWN_ROUTES = [
    "/chunks", "/graph", "/dashboard", "/search", "/tags", "/codebases",
    "/workspaces", "/templates", "/context", "/knowledge-health", "/coverage",
    "/plans", "/requirements", "/import", "/settings", "/activity",
    "/vocabulary", "/docs", "/compare", "/login"
];

interface LinkInfo {
    chunkId: string;
    chunkTitle: string;
    text: string;
    url: string;
    status: "ok" | "external" | "broken";
}

const LINK_REGEX = /\[([^\]]*)\]\(([^)]+)\)/g;

function extractLinks(content: string, chunkId: string, chunkTitle: string): LinkInfo[] {
    const links: LinkInfo[] = [];
    let match: RegExpExecArray | null;
    while ((match = LINK_REGEX.exec(content)) !== null) {
        const text = match[1];
        const url = match[2];

        let status: LinkInfo["status"];
        if (url.startsWith("http://") || url.startsWith("https://")) {
            status = "external";
        } else if (url.startsWith("/")) {
            const basePath = url.split("?")[0].split("#")[0];
            const isKnown = KNOWN_ROUTES.some(r => basePath === r || basePath.startsWith(r + "/"));
            status = isKnown ? "ok" : "broken";
        } else if (url.startsWith("#")) {
            status = "ok";
        } else {
            status = "broken";
        }

        links.push({ chunkId, chunkTitle, text, url, status });
    }
    return links;
}

const STATUS_STYLES: Record<string, { label: string; className: string }> = {
    ok: { label: "OK", className: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400" },
    external: { label: "External", className: "bg-blue-500/10 text-blue-600 border-blue-500/30 dark:text-blue-400" },
    broken: { label: "Broken", className: "bg-red-500/10 text-red-600 border-red-500/30 dark:text-red-400" }
};

export function BrokenLinkChecker() {
    const chunksQuery = useQuery({
        queryKey: ["chunks-link-check"],
        queryFn: async () => {
            const res = unwrapEden(await api.api.chunks.get({ query: { limit: "500" } }));
            return (res.chunks ?? []) as Array<{ id: string; title: string; content: string }>;
        },
        staleTime: 120_000
    });

    const links = useMemo(() => {
        if (!chunksQuery.data) return [];
        return chunksQuery.data.flatMap(c => extractLinks(c.content, c.id, c.title));
    }, [chunksQuery.data]);

    const brokenCount = links.filter(l => l.status === "broken").length;
    const externalCount = links.filter(l => l.status === "external").length;

    if (chunksQuery.isLoading) return <PageLoading count={2} />;

    // Only show if there are links to display
    if (links.length === 0) return null;

    // Sort: broken first, then external, then ok
    const sortedLinks = [...links].sort((a, b) => {
        const order = { broken: 0, external: 1, ok: 2 };
        return order[a.status] - order[b.status];
    });

    return (
        <Card>
            <CardPanel className="p-6">
                <div className="mb-3 flex items-center gap-2">
                    <LinkIcon className="size-4 text-indigo-500" />
                    <h2 className="text-lg font-semibold">Content Links</h2>
                    <Badge variant="secondary">{links.length}</Badge>
                    {brokenCount > 0 && (
                        <Badge variant="destructive">{brokenCount} broken</Badge>
                    )}
                    {externalCount > 0 && (
                        <Badge variant="outline">{externalCount} external</Badge>
                    )}
                </div>
                <p className="text-muted-foreground mb-4 text-sm">
                    Markdown links found in chunk content. Broken links point to unrecognized internal paths.
                    External links should be checked manually.
                </p>
                <div className="max-h-96 overflow-y-auto">
                    <table className="w-full text-sm">
                        <thead className="text-muted-foreground border-b text-left text-xs">
                            <tr>
                                <th className="pb-2 pr-3">Chunk</th>
                                <th className="pb-2 pr-3">Link Text</th>
                                <th className="pb-2 pr-3">URL</th>
                                <th className="pb-2">Status</th>
                            </tr>
                        </thead>
                        <tbody className="divide-y">
                            {sortedLinks.map((link, i) => {
                                const style = STATUS_STYLES[link.status];
                                return (
                                    <tr key={`${link.chunkId}-${i}`}>
                                        <td className="py-2 pr-3">
                                            <Link
                                                to="/chunks/$chunkId"
                                                params={{ chunkId: link.chunkId }}
                                                className="hover:underline font-medium"
                                            >
                                                {link.chunkTitle}
                                            </Link>
                                        </td>
                                        <td className="text-muted-foreground py-2 pr-3">{link.text || "(empty)"}</td>
                                        <td className="py-2 pr-3">
                                            <code className="text-xs break-all">{link.url}</code>
                                        </td>
                                        <td className="py-2">
                                            <Badge variant="outline" className={style.className}>
                                                {style.label}
                                            </Badge>
                                        </td>
                                    </tr>
                                );
                            })}
                        </tbody>
                    </table>
                </div>
            </CardPanel>
        </Card>
    );
}
