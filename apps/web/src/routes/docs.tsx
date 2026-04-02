import { createFileRoute, useSearch } from "@tanstack/react-router";
import { ExternalLink, Globe, Book } from "lucide-react";
import { useEffect, useState } from "react";

import { DocumentBrowser } from "@/features/documents/document-browser";
import { getUser } from "@/functions/get-user";

export const Route = createFileRoute("/docs")({
    component: DocsPage,
    validateSearch: (search: Record<string, unknown>): { tab?: string } => ({
        tab: (search.tab as string) ?? undefined
    }),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

function DocsPage() {
    const search = useSearch({ from: "/docs" });
    const [tab, setTab] = useState<"docs" | "api">(
        search.tab === "api" ? "api" : "docs"
    );

    useEffect(() => {
        if (search.tab === "api") setTab("api");
        if (search.tab === "docs") setTab("docs");
    }, [search.tab]);

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight">Documentation</h1>
                    <p className="text-muted-foreground mt-0.5 text-sm">Browse imported documentation and API reference.</p>
                </div>
            </div>

            <div className="border-border mb-6 flex gap-1 border-b">
                <button
                    onClick={() => setTab("docs")}
                    className={`-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
                        tab === "docs"
                            ? "border-foreground text-foreground"
                            : "text-muted-foreground hover:text-foreground border-transparent"
                    }`}
                >
                    <span className="flex items-center gap-2">
                        <Book className="size-4" />
                        Documentation
                    </span>
                </button>
                <button
                    onClick={() => setTab("api")}
                    className={`-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
                        tab === "api"
                            ? "border-foreground text-foreground"
                            : "text-muted-foreground hover:text-foreground border-transparent"
                    }`}
                >
                    <span className="flex items-center gap-2">
                        <Globe className="size-4" />
                        API Reference
                    </span>
                </button>
            </div>

            {tab === "docs" && <DocumentBrowser />}

            {tab === "api" && (
                <div className="space-y-3">
                    <div className="flex items-center justify-end">
                        <a
                            href="http://localhost:3000/docs"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-sm transition-colors"
                        >
                            Open in new tab
                            <ExternalLink className="size-3.5" />
                        </a>
                    </div>
                    <div className="rounded-lg border overflow-hidden" style={{ height: "calc(100vh - 200px)" }}>
                        <iframe
                            src="http://localhost:3000/docs"
                            className="w-full h-full border-0"
                            title="Fubbik API Documentation"
                        />
                    </div>
                </div>
            )}
        </div>
    );
}
