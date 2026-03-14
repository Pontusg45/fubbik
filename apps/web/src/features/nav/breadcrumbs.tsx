import { Link, useMatches } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";
import { Fragment } from "react";

interface BreadcrumbSegment {
    label: string;
    to?: string;
}

const ROUTE_LABELS: Record<string, string> = {
    dashboard: "Dashboard",
    chunks: "Chunks",
    graph: "Graph",
    tags: "Tags",
    "knowledge-health": "Health",
    codebases: "Codebases",
    requirements: "Requirements",
    vocabulary: "Vocabulary",
    templates: "Templates",
    new: "New",
    edit: "Edit"
};

function truncate(str: string, max: number) {
    if (str.length <= max) return str;
    return `${str.slice(0, max)}...`;
}

export function Breadcrumbs() {
    const matches = useMatches();

    // Build segments from matched routes, skipping root
    const segments: BreadcrumbSegment[] = [{ label: "Fubbik", to: "/" }];

    // Get the full pathname from the last match
    const lastMatch = matches[matches.length - 1];
    if (!lastMatch) return null;

    const pathname = lastMatch.pathname;

    // Skip breadcrumbs on the index/root page
    if (pathname === "/") return null;

    const parts = pathname.split("/").filter(Boolean);

    for (let i = 0; i < parts.length; i++) {
        const part = parts[i]!;
        const path = `/${parts.slice(0, i + 1).join("/")}`;
        const isLast = i === parts.length - 1;

        // Check if this is a known label
        const label = ROUTE_LABELS[part];

        if (label) {
            segments.push({
                label,
                to: isLast ? undefined : path
            });
        } else {
            // Dynamic segment - try to get a meaningful label
            // Check if previous part was "chunks" or "requirements" to look for title in loader data
            const parentPart = parts[i - 1];
            let dynamicLabel = part;

            if (parentPart === "chunks") {
                // Try to find chunk title from the matching route's loader data
                const chunkMatch = matches.find(
                    m => m.loaderData && typeof m.loaderData === "object" && "chunk" in (m.loaderData as Record<string, unknown>)
                );
                if (chunkMatch) {
                    const loaderData = chunkMatch.loaderData as unknown as { chunk?: { title?: string } };
                    if (loaderData.chunk?.title) {
                        dynamicLabel = truncate(loaderData.chunk.title, 30);
                    }
                }
            } else if (parentPart === "requirements_" || parentPart === "requirements") {
                const reqMatch = matches.find(
                    m =>
                        m.loaderData &&
                        typeof m.loaderData === "object" &&
                        "requirement" in (m.loaderData as Record<string, unknown>)
                );
                if (reqMatch) {
                    const loaderData = reqMatch.loaderData as unknown as { requirement?: { title?: string } };
                    if (loaderData.requirement?.title) {
                        dynamicLabel = truncate(loaderData.requirement.title, 30);
                    }
                }
            }

            segments.push({
                label: dynamicLabel,
                to: isLast ? undefined : path
            });
        }
    }

    // Don't show breadcrumbs if we only have "Fubbik > X" (single-level pages are obvious)
    if (segments.length <= 2) return null;

    return (
        <nav aria-label="Breadcrumb" className="container mx-auto px-4 py-2">
            <ol className="flex items-center gap-1 text-sm">
                {segments.map((segment, index) => {
                    const isLast = index === segments.length - 1;
                    return (
                        <Fragment key={`${segment.label}-${index}`}>
                            {index > 0 && <ChevronRight className="text-muted-foreground size-3 shrink-0" />}
                            <li>
                                {segment.to && !isLast ? (
                                    <Link to={segment.to} className="text-muted-foreground hover:text-foreground transition-colors">
                                        {segment.label}
                                    </Link>
                                ) : (
                                    <span className="text-foreground font-medium">{segment.label}</span>
                                )}
                            </li>
                        </Fragment>
                    );
                })}
            </ol>
        </nav>
    );
}
