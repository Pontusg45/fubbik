import { useMutation } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Plus, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type AnalyzeKind = "chunk" | "file" | "risk" | "assumption" | "question";

interface AnalyzeItem {
    id: string;
    kind: AnalyzeKind;
    chunkId: string | null;
    filePath: string | null;
    text: string | null;
    metadata: Record<string, unknown>;
}

interface AnalyzeGroups {
    chunk: AnalyzeItem[];
    file: AnalyzeItem[];
    risk: AnalyzeItem[];
    assumption: AnalyzeItem[];
    question: AnalyzeItem[];
}

const KIND_LABELS: Record<AnalyzeKind, string> = {
    chunk: "Chunks",
    file: "Files",
    risk: "Risks",
    assumption: "Assumptions",
    question: "Questions"
};

const KIND_ORDER: AnalyzeKind[] = ["chunk", "file", "risk", "assumption", "question"];

export interface PlanAnalyzeSectionProps {
    planId: string;
    analyze: AnalyzeGroups;
    onUpdate: () => void;
}

export function PlanAnalyzeSection({ planId, analyze, onUpdate }: PlanAnalyzeSectionProps) {
    const [openKinds, setOpenKinds] = useState<Set<AnalyzeKind>>(new Set(KIND_ORDER));

    const toggle = (k: AnalyzeKind) =>
        setOpenKinds(prev => {
            const next = new Set(prev);
            if (next.has(k)) next.delete(k);
            else next.add(k);
            return next;
        });

    return (
        <section className="space-y-3">
            <h2 className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">Analyze</h2>
            <div className="space-y-2 rounded-md border p-2">
                {KIND_ORDER.map(kind => (
                    <AnalyzeKindBlock
                        key={kind}
                        kind={kind}
                        items={analyze[kind]}
                        planId={planId}
                        open={openKinds.has(kind)}
                        onToggle={() => toggle(kind)}
                        onUpdate={onUpdate}
                    />
                ))}
            </div>
        </section>
    );
}

function AnalyzeKindBlock({
    kind,
    items,
    planId,
    open,
    onToggle,
    onUpdate
}: {
    kind: AnalyzeKind;
    items: AnalyzeItem[];
    planId: string;
    open: boolean;
    onToggle: () => void;
    onUpdate: () => void;
}) {
    const [adding, setAdding] = useState(false);
    const [draftText, setDraftText] = useState("");
    const [draftFilePath, setDraftFilePath] = useState("");
    const [draftSeverity, setDraftSeverity] = useState<"low" | "medium" | "high">("medium");

    const addMutation = useMutation({
        mutationFn: async () => {
            const body: Record<string, unknown> = { kind };
            if (kind === "file") {
                body.filePath = draftFilePath;
                body.text = draftText;
            } else if (kind === "risk") {
                body.text = draftText;
                body.metadata = { severity: draftSeverity };
            } else if (kind === "assumption") {
                body.text = draftText;
                body.metadata = { verified: false };
            } else if (kind === "question") {
                body.text = draftText;
                body.metadata = { answered: false };
            } else if (kind === "chunk") {
                body.chunkId = draftText;
            }
            return unwrapEden(await api.api.plans({ id: planId }).analyze.post(body));
        },
        onSuccess: () => {
            setAdding(false);
            setDraftText("");
            setDraftFilePath("");
            onUpdate();
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (itemId: string) =>
            unwrapEden(await api.api.plans({ id: planId }).analyze({ itemId }).delete()),
        onSuccess: () => onUpdate()
    });

    return (
        <div className="rounded border">
            <button
                type="button"
                onClick={onToggle}
                className="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 px-2 py-1.5 text-xs font-semibold tracking-wider uppercase"
            >
                {open ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                {KIND_LABELS[kind]}
                <span className="text-muted-foreground/60 ml-1 font-mono">({items.length})</span>
                <div className="ml-auto" />
                {open && (
                    <span
                        role="button"
                        tabIndex={0}
                        onClick={e => {
                            e.stopPropagation();
                            setAdding(a => !a);
                        }}
                        className="text-muted-foreground hover:text-foreground"
                    >
                        <Plus className="size-3" />
                    </span>
                )}
            </button>
            {open && (
                <div className="space-y-1 px-2 pb-2">
                    {adding && (
                        <div className="bg-muted/30 space-y-2 rounded border p-2">
                            {kind === "file" && (
                                <Input
                                    placeholder="File path, e.g. src/foo.ts"
                                    value={draftFilePath}
                                    onChange={e => setDraftFilePath(e.target.value)}
                                    className="h-7 text-xs"
                                />
                            )}
                            <Input
                                autoFocus
                                placeholder={
                                    kind === "chunk"
                                        ? "Chunk ID"
                                        : kind === "risk"
                                          ? "Describe the risk"
                                          : kind === "assumption"
                                            ? "What are you assuming?"
                                            : kind === "question"
                                              ? "What's the open question?"
                                              : "Note"
                                }
                                value={draftText}
                                onChange={e => setDraftText(e.target.value)}
                                className="h-7 text-xs"
                            />
                            {kind === "risk" && (
                                <select
                                    value={draftSeverity}
                                    onChange={e => setDraftSeverity(e.target.value as any)}
                                    className="bg-background h-7 w-full rounded border px-2 text-xs"
                                >
                                    <option value="low">Low</option>
                                    <option value="medium">Medium</option>
                                    <option value="high">High</option>
                                </select>
                            )}
                            <div className="flex gap-2">
                                <Button size="sm" onClick={() => addMutation.mutate()}>
                                    Add
                                </Button>
                                <Button size="sm" variant="ghost" onClick={() => setAdding(false)}>
                                    Cancel
                                </Button>
                            </div>
                        </div>
                    )}
                    {items.map(item => (
                        <div key={item.id} className="hover:bg-muted/40 flex items-center gap-2 rounded px-2 py-1 text-xs">
                            <span className="flex-1 truncate">
                                {kind === "file" && item.filePath ? (
                                    <>
                                        <span className="font-mono">{item.filePath}</span>
                                        {item.text && <span className="text-muted-foreground ml-2">— {item.text}</span>}
                                    </>
                                ) : (
                                    (item.text ?? item.chunkId ?? "(empty)")
                                )}
                            </span>
                            {kind === "risk" && (
                                <span className="text-muted-foreground text-[9px] uppercase">
                                    {(item.metadata as any)?.severity ?? "medium"}
                                </span>
                            )}
                            <button
                                type="button"
                                onClick={() => deleteMutation.mutate(item.id)}
                                className="text-muted-foreground hover:text-foreground"
                                aria-label="Delete"
                            >
                                <X className="size-3" />
                            </button>
                        </div>
                    ))}
                    {items.length === 0 && !adding && <div className="text-muted-foreground py-2 text-center text-[10px]">None yet</div>}
                </div>
            )}
        </div>
    );
}
