import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Scissors } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogClose,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
    DialogTrigger
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface SplitSection {
    title: string;
    content: string;
    included: boolean;
}

function splitByHeadings(title: string, content: string): SplitSection[] {
    const lines = content.split("\n");
    const sections: SplitSection[] = [];
    let currentTitle = title;
    let currentLines: string[] = [];

    for (const line of lines) {
        const headingMatch = line.match(/^(#{1,3})\s+(.+)$/);
        if (headingMatch) {
            // Save previous section if it has content
            const prevContent = currentLines.join("\n").trim();
            if (prevContent) {
                sections.push({ title: currentTitle, content: prevContent, included: true });
            }
            currentTitle = headingMatch[2]!;
            currentLines = [];
        } else {
            currentLines.push(line);
        }
    }

    // Save last section
    const lastContent = currentLines.join("\n").trim();
    if (lastContent) {
        sections.push({ title: currentTitle, content: lastContent, included: true });
    }

    return sections;
}

export function SplitChunkDialog({
    chunkId,
    title,
    content,
    type,
    tags
}: {
    chunkId: string;
    title: string;
    content: string;
    type: string;
    tags: string[];
}) {
    const [sections, setSections] = useState<SplitSection[]>(() => splitByHeadings(title, content));
    const queryClient = useQueryClient();

    const includedCount = sections.filter(s => s.included).length;
    const canSplit = sections.length >= 2;

    const splitMutation = useMutation({
        mutationFn: async () => {
            const included = sections.filter(s => s.included);
            const excluded = sections.filter(s => !s.included);
            const created: { id: string; title: string }[] = [];

            for (const section of included) {
                const result = unwrapEden(
                    await api.api.chunks.post({
                        title: section.title,
                        content: section.content,
                        type,
                        tags
                    })
                ) as { id: string };
                created.push({ id: result.id, title: section.title });
                await api.api.connections.post({
                    sourceId: result.id,
                    targetId: chunkId,
                    relation: "part_of"
                });
            }

            // Update original chunk: keep only excluded sections content, add index of split-out parts
            const keepContent = excluded.map(s => `## ${s.title}\n\n${s.content}`).join("\n\n");
            const indexContent = created.map(c => `- ${c.title}`).join("\n");
            const newContent = [keepContent, `## Sections\n\n${indexContent}`].filter(Boolean).join("\n\n");

            await api.api.chunks({ id: chunkId }).patch({ content: newContent });

            return created;
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success(`Split into ${includedCount} chunks`);
        },
        onError: () => {
            toast.error("Failed to split chunk");
        }
    });

    function toggleSection(index: number) {
        setSections(prev => prev.map((s, i) => (i === index ? { ...s, included: !s.included } : s)));
    }

    if (!canSplit) {
        return null;
    }

    return (
        <Dialog
            onOpenChange={open => {
                if (open) setSections(splitByHeadings(title, content));
            }}
        >
            <DialogTrigger
                render={
                    <Button variant="outline" size="sm">
                        <Scissors className="size-3.5" />
                        Split
                    </Button>
                }
            />
            <DialogPopup className="max-w-2xl">
                <DialogHeader>
                    <DialogTitle>Split Chunk</DialogTitle>
                    <DialogDescription>
                        Split into {sections.length} sections by headings. Toggle sections to include or exclude them.
                    </DialogDescription>
                </DialogHeader>
                <DialogPanel>
                    <div className="space-y-3">
                        {sections.map((section, i) => (
                            <button
                                key={i}
                                type="button"
                                onClick={() => toggleSection(i)}
                                className={`w-full rounded-lg border p-4 text-left transition-colors ${
                                    section.included ? "border-primary/30 bg-primary/5" : "border-muted bg-muted/30 opacity-50"
                                }`}
                            >
                                <div className="mb-2 flex items-center justify-between">
                                    <span className="text-sm font-semibold">{section.title}</span>
                                    <Badge variant={section.included ? "default" : "outline"} size="sm">
                                        {section.included ? "Included" : "Excluded"}
                                    </Badge>
                                </div>
                                <Separator className="my-2" />
                                <div className="prose dark:prose-invert prose-sm max-h-32 max-w-none overflow-hidden text-xs">
                                    <MarkdownRenderer>
                                        {section.content.slice(0, 300) + (section.content.length > 300 ? "..." : "")}
                                    </MarkdownRenderer>
                                </div>
                            </button>
                        ))}
                    </div>
                </DialogPanel>
                <DialogFooter>
                    <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
                    <Button onClick={() => splitMutation.mutate()} disabled={splitMutation.isPending || includedCount < 1}>
                        {splitMutation.isPending ? "Splitting..." : `Split into ${includedCount} chunks`}
                    </Button>
                </DialogFooter>
            </DialogPopup>
        </Dialog>
    );
}
