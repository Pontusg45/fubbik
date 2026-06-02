import {
    Activity,
    Blocks,
    BookOpen,
    Clock,
    ClipboardCheck,
    FileCode,
    FileText,
    Globe,
    Hash,
    LayoutDashboard,
    ListChecks,
    Network,
    Plus,
    Server,
    Settings,
    Tags
} from "lucide-react";
import React from "react";

import type { CommandItem, RecentPage } from "./command-types";

// ---------------------------------------------------------------------------
// Static constant definitions
// ---------------------------------------------------------------------------

export const PAGE_ITEMS: Array<{ id: string; title: string; path: string; icon: React.ReactNode }> = [
    { id: "page-dashboard", title: "Dashboard", path: "/dashboard", icon: React.createElement(LayoutDashboard, { className: "size-4" }) },
    { id: "page-chunks", title: "Chunks", path: "/chunks", icon: React.createElement(Blocks, { className: "size-4" }) },
    { id: "page-graph", title: "Graph", path: "/graph", icon: React.createElement(Network, { className: "size-4" }) },
    { id: "page-tags", title: "Tags", path: "/tags", icon: React.createElement(Tags, { className: "size-4" }) },
    { id: "page-spaces", title: "Spaces", path: "/spaces", icon: React.createElement(Server, { className: "size-4" }) },
    { id: "page-templates", title: "Templates", path: "/templates", icon: React.createElement(FileCode, { className: "size-4" }) },
    { id: "page-health", title: "Health", path: "/knowledge-health", icon: React.createElement(Activity, { className: "size-4" }) },
    { id: "page-requirements", title: "Requirements", path: "/requirements", icon: React.createElement(FileText, { className: "size-4" }) },
    { id: "page-vocabulary", title: "Vocabulary", path: "/vocabulary", icon: React.createElement(BookOpen, { className: "size-4" }) }
];

export const ACTION_ITEMS: Array<{
    id: string;
    title: string;
    path: string;
    search?: Record<string, string>;
    icon: React.ReactNode;
    /** If set, this action triggers a sub-mode instead of navigating */
    subMode?: string;
}> = [
    { id: "action-new-chunk", title: "New Chunk", path: "/chunks/new", icon: React.createElement(Plus, { className: "size-4" }) },
    {
        id: "action-new-note",
        title: "New Note",
        path: "/chunks/new",
        search: { type: "note" },
        icon: React.createElement(FileText, { className: "size-4" })
    },
    {
        id: "action-new-document",
        title: "New Document",
        path: "/chunks/new",
        search: { type: "document" },
        icon: React.createElement(FileText, { className: "size-4" })
    },
    {
        id: "action-new-requirement",
        title: "New Requirement",
        path: "/requirements/new",
        icon: React.createElement(Plus, { className: "size-4" })
    },
    { id: "action-new-plan", title: "New Plan", path: "/plans/new", icon: React.createElement(Plus, { className: "size-4" }) },
    {
        id: "action-switch-space",
        title: "Switch Space",
        path: "",
        icon: React.createElement(Settings, { className: "size-4" }),
        subMode: "space"
    },
    {
        id: "action-view-health",
        title: "View Health",
        path: "/knowledge-health",
        icon: React.createElement(Activity, { className: "size-4" })
    }
];

// ---------------------------------------------------------------------------
// Pure builder functions
// ---------------------------------------------------------------------------

export function buildSpaceItems(
    spaces: Array<{ id: string; name: string; remoteUrl: string | null }>,
    lowerQuery: string,
    onSelect: (id: string) => void
): CommandItem[] {
    const filtered = spaces.filter(c => !lowerQuery || c.name.toLowerCase().includes(lowerQuery));
    return filtered.map(s => ({
        id: `sp-${s.id}`,
        title: s.name,
        group: "Spaces",
        icon: React.createElement(Server, { className: "size-4" }),
        badge: s.remoteUrl ? "git" : undefined,
        onSelect: () => onSelect(s.id)
    }));
}

export function buildChunkQuickOpenItems(
    chunks: Array<{ id: string; title: string; type: string }>,
    lowerQuery: string,
    onSelect: (id: string) => void
): CommandItem[] {
    const filtered = chunks
        .filter(c => {
            if (!lowerQuery) return true;
            const title = c.title.toLowerCase();
            let qi = 0;
            for (let i = 0; i < title.length && qi < lowerQuery.length; i++) {
                if (title[i] === lowerQuery[qi]) qi++;
            }
            return qi === lowerQuery.length;
        })
        .slice(0, 20);
    return filtered.map(chunk => ({
        id: `qo-${chunk.id}`,
        title: chunk.title,
        group: "Chunks",
        icon: React.createElement(Blocks, { className: "size-4" }),
        badge: chunk.type,
        onSelect: () => onSelect(chunk.id)
    }));
}

export function buildFederatedItems(
    chunks: Array<{ id: string; title?: string | null; codebaseName?: string | null }>,
    onSelect: (id: string) => void
): CommandItem[] {
    return chunks.map(chunk => ({
        id: `fed-${chunk.id}`,
        title: chunk.title ?? `Chunk ${chunk.id.slice(0, 8)}`,
        group: "All Spaces",
        icon: React.createElement(Globe, { className: "size-4" }),
        badge: chunk.codebaseName ?? "Global",
        onSelect: () => onSelect(chunk.id)
    }));
}

export function buildTagItems(
    tags: Array<{ id: string; name: string }>,
    tagQuery: string,
    onSelect: (name: string) => void
): CommandItem[] {
    const filtered = tags.filter(t => !tagQuery || t.name.toLowerCase().includes(tagQuery));
    return filtered.slice(0, 10).map(tag => ({
        id: `tag-${tag.id}`,
        title: `#${tag.name}`,
        group: "Tags",
        icon: React.createElement(Hash, { className: "size-4" }),
        onSelect: () => onSelect(tag.name)
    }));
}

export function buildRecentPageItems(recentPages: RecentPage[], onSelect: (path: string) => void): CommandItem[] {
    return recentPages.map(page => ({
        id: `recent-page-${page.path}`,
        title: page.title,
        group: "Recent",
        icon: React.createElement(Clock, { className: "size-4" }),
        badge: "Page",
        onSelect: () => onSelect(page.path)
    }));
}

export function buildRecentChunkItems(
    recentChunks: Array<{ chunk: Record<string, unknown> } | null | undefined>,
    onSelect: (id: string) => void
): CommandItem[] {
    const items: CommandItem[] = [];
    for (const item of recentChunks) {
        if (!item) continue;
        const id = item.chunk.id as string;
        const title = item.chunk.title as string | undefined;
        items.push({
            id: `recent-${id}`,
            title: title ?? `Chunk ${id.slice(0, 8)}`,
            group: "Recent",
            icon: React.createElement(Clock, { className: "size-4" }),
            onSelect: () => onSelect(id)
        });
    }
    return items;
}

export function buildPageItems(lowerQuery: string, onSelect: (path: string) => void): CommandItem[] {
    const filtered = PAGE_ITEMS.filter(p => !lowerQuery || p.title.toLowerCase().includes(lowerQuery));
    return filtered.map(page => ({
        id: page.id,
        title: page.title,
        group: "Pages",
        icon: page.icon,
        onSelect: () => onSelect(page.path)
    }));
}

export function buildChunkSearchItems(chunks: Array<{ id: string; title: string }>, onSelect: (id: string) => void): CommandItem[] {
    return chunks.map(chunk => ({
        id: `chunk-${chunk.id}`,
        title: chunk.title ?? `Chunk ${chunk.id.slice(0, 8)}`,
        group: "Chunks",
        icon: React.createElement(Blocks, { className: "size-4" }),
        onSelect: () => onSelect(chunk.id)
    }));
}

export function buildRequirementItems(
    requirements: Array<{ id: string; title: string; status: string }>,
    onSelect: (id: string) => void
): CommandItem[] {
    return requirements.slice(0, 5).map(req => ({
        id: `req-${req.id}`,
        title: req.title,
        group: "Requirements",
        icon: React.createElement(ClipboardCheck, { className: "size-4" }),
        badge: req.status,
        onSelect: () => onSelect(req.id)
    }));
}

export function buildPlanItems(
    plans: Array<{ id: string; title: string; status: string }>,
    lowerQuery: string,
    onSelect: (id: string) => void
): CommandItem[] {
    const filtered = plans.filter(p => p.title.toLowerCase().includes(lowerQuery)).slice(0, 5);
    return filtered.map(plan => ({
        id: `plan-${plan.id}`,
        title: plan.title,
        group: "Plans",
        icon: React.createElement(ListChecks, { className: "size-4" }),
        badge: plan.status,
        onSelect: () => onSelect(plan.id)
    }));
}

export function buildActionItems(
    lowerQuery: string,
    onSelect: (action: { path: string; search?: Record<string, string>; subMode?: string }) => void
): CommandItem[] {
    const filtered = ACTION_ITEMS.filter(a => !lowerQuery || a.title.toLowerCase().includes(lowerQuery));
    return filtered.map(action => ({
        id: action.id,
        title: action.title,
        group: "Actions",
        icon: action.icon,
        onSelect: () => onSelect(action)
    }));
}

export function groupItems(items: CommandItem[]): Map<string, { items: CommandItem[]; startIndex: number }> {
    const groups = new Map<string, { items: CommandItem[]; startIndex: number }>();
    let idx = 0;
    for (const item of items) {
        if (!groups.has(item.group)) {
            groups.set(item.group, { items: [], startIndex: idx });
        }
        groups.get(item.group)!.items.push(item);
        idx++;
    }
    return groups;
}
