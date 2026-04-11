import { History as HistoryIcon, Layers, MessageSquare, Network } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Sheet, SheetContent, SheetHeader, SheetPanel, SheetTitle } from "@/components/ui/sheet";
import { ChunkComments } from "@/features/chunks/chunk-comments";
import { VersionHistory } from "@/features/chunks/version-history";

import type { AppliesTo, FileReference } from "./more-context-context-tab";
import { MoreContextContextTab } from "./more-context-context-tab";
import type { ConnectionItem, TagItem } from "./more-context-links-tab";
import { MoreContextLinksTab } from "./more-context-links-tab";

export type DrawerTab = "links" | "context" | "comments" | "history";

const VALID_TABS: DrawerTab[] = ["links", "context", "comments", "history"];
const STORAGE_KEY = "chunk-detail-drawer-tab";

export interface MoreContextDrawerProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    chunkId: string;
    chunkTitle: string;
    outgoing: ConnectionItem[];
    incoming: ConnectionItem[];
    tags: TagItem[];
    appliesTo?: AppliesTo[];
    fileReferences?: FileReference[];
    initialTab?: DrawerTab;
}

export function MoreContextDrawer({
    open,
    onOpenChange,
    chunkId,
    chunkTitle,
    outgoing,
    incoming,
    tags,
    appliesTo,
    fileReferences,
    initialTab,
}: MoreContextDrawerProps) {
    const [tab, setTab] = useState<DrawerTab>(initialTab ?? "links");

    useEffect(() => {
        if (initialTab) {
            setTab(initialTab);
            return;
        }
        try {
            const stored = sessionStorage.getItem(STORAGE_KEY) as DrawerTab | null;
            if (stored && VALID_TABS.includes(stored)) {
                setTab(stored);
            }
        } catch {
            // ignore
        }
    }, [initialTab]);

    function changeTab(next: DrawerTab) {
        setTab(next);
        try {
            sessionStorage.setItem(STORAGE_KEY, next);
        } catch {
            // ignore
        }
    }

    const linkCount = outgoing.length + incoming.length;
    const contextCount = (appliesTo?.length ?? 0) + (fileReferences?.length ?? 0);

    return (
        <Sheet open={open} onOpenChange={onOpenChange}>
            <SheetContent side="right">
                <SheetHeader>
                    <SheetTitle>More context</SheetTitle>
                    <div className="flex gap-1 border-b pb-2">
                        <TabButton active={tab === "links"} onClick={() => changeTab("links")}>
                            <Network className="size-3.5" />
                            Links
                            {linkCount > 0 && (
                                <Badge variant="secondary" size="sm">
                                    {linkCount}
                                </Badge>
                            )}
                        </TabButton>
                        <TabButton active={tab === "context"} onClick={() => changeTab("context")}>
                            <Layers className="size-3.5" />
                            Context
                            {contextCount > 0 && (
                                <Badge variant="secondary" size="sm">
                                    {contextCount}
                                </Badge>
                            )}
                        </TabButton>
                        <TabButton active={tab === "comments"} onClick={() => changeTab("comments")}>
                            <MessageSquare className="size-3.5" />
                            Comments
                        </TabButton>
                        <TabButton active={tab === "history"} onClick={() => changeTab("history")}>
                            <HistoryIcon className="size-3.5" />
                            History
                        </TabButton>
                    </div>
                </SheetHeader>

                <SheetPanel>
                    {tab === "links" && (
                        <MoreContextLinksTab
                            chunkId={chunkId}
                            chunkTitle={chunkTitle}
                            outgoing={outgoing}
                            incoming={incoming}
                            tags={tags}
                        />
                    )}
                    {tab === "context" && (
                        <MoreContextContextTab
                            chunkId={chunkId}
                            appliesTo={appliesTo}
                            fileReferences={fileReferences}
                        />
                    )}
                    {tab === "comments" && <ChunkComments chunkId={chunkId} />}
                    {tab === "history" && <VersionHistory chunkId={chunkId} />}
                </SheetPanel>
            </SheetContent>
        </Sheet>
    );
}

function TabButton({
    active,
    onClick,
    children,
}: {
    active: boolean;
    onClick: () => void;
    children: React.ReactNode;
}) {
    return (
        <button
            type="button"
            onClick={onClick}
            className={`flex items-center gap-1.5 rounded px-3 py-1.5 text-xs font-medium transition-colors ${
                active ? "bg-muted text-foreground" : "text-muted-foreground hover:bg-muted/50"
            }`}
        >
            {children}
        </button>
    );
}
