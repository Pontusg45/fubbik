import { useMutation } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type TaskStatus = "pending" | "in_progress" | "done" | "skipped" | "blocked";

export interface AcceptanceCriterion {
    text: string;
    done: boolean;
}

export interface TaskChunk {
    id: string;
    chunkId: string;
    relation: string;
    chunkTitle: string | null;
    chunkType: string | null;
}

export interface Task {
    id: string;
    title: string;
    description: string | null;
    acceptanceCriteria: AcceptanceCriterion[];
    status: TaskStatus;
    chunks: TaskChunk[];
}

const STATUS_LABEL: Record<TaskStatus, string> = {
    pending: "Pending",
    in_progress: "In progress",
    blocked: "Blocked",
    done: "Done",
    skipped: "Skipped",
};

const STATUS_DOT_CLASS: Record<TaskStatus, string> = {
    pending: "bg-slate-400",
    in_progress: "bg-blue-500",
    blocked: "bg-amber-500",
    done: "bg-emerald-500",
    skipped: "bg-zinc-500",
};

export interface PlanTaskCardProps {
    planId: string;
    task: Task;
    onUpdate: () => void;
}

export function PlanTaskCard({ planId, task, onUpdate }: PlanTaskCardProps) {
    const [expanded, setExpanded] = useState(false);
    const [titleDraft, setTitleDraft] = useState(task.title);
    const [editingTitle, setEditingTitle] = useState(false);
    const [descDraft, setDescDraft] = useState(task.description ?? "");

    useEffect(() => { setTitleDraft(task.title); }, [task.title]);
    useEffect(() => { setDescDraft(task.description ?? ""); }, [task.description]);

    const updateMutation = useMutation({
        mutationFn: async (patch: Record<string, unknown>) =>
            unwrapEden(await (api.api as any).plans[planId].tasks[task.id].patch(patch)),
        onSuccess: () => onUpdate(),
    });

    const deleteMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).plans[planId].tasks[task.id].delete()),
        onSuccess: () => onUpdate(),
    });

    const setStatus = (next: TaskStatus) => {
        if (next !== task.status) updateMutation.mutate({ status: next });
    };

    const toggleCriterion = (idx: number, done: boolean) => {
        const next = task.acceptanceCriteria.map((c, i) => (i === idx ? { ...c, done } : c));
        updateMutation.mutate({ acceptanceCriteria: next });
    };

    const saveDescription = () => {
        if (descDraft !== (task.description ?? "")) {
            updateMutation.mutate({ description: descDraft || null });
        }
    };

    return (
        <div className="rounded-md border bg-card">
            <div className="flex items-start gap-3 p-3">
                {/* Status dot + select; clicking the dot rotates done/pending for fast toggling */}
                <Select value={task.status} onValueChange={v => { if (v) setStatus(v as TaskStatus); }}>
                    <SelectTrigger
                        size="sm"
                        className="mt-0.5 h-auto min-h-0 w-auto border-0 bg-transparent p-0 shadow-none hover:opacity-80"
                        title={`Status: ${STATUS_LABEL[task.status]}`}
                    >
                        <SelectValue>
                            <span className={`mt-0.5 inline-block size-3 rounded-full ${STATUS_DOT_CLASS[task.status]}`} />
                        </SelectValue>
                    </SelectTrigger>
                    <SelectPopup>
                        {(Object.keys(STATUS_LABEL) as TaskStatus[]).map(s => (
                            <SelectItem key={s} value={s}>
                                <span className={`mr-2 inline-block size-2.5 rounded-full ${STATUS_DOT_CLASS[s]}`} />
                                {STATUS_LABEL[s]}
                            </SelectItem>
                        ))}
                    </SelectPopup>
                </Select>

                <div className="flex-1">
                    {editingTitle ? (
                        <input
                            autoFocus
                            value={titleDraft}
                            onChange={e => setTitleDraft(e.target.value)}
                            onBlur={() => {
                                setEditingTitle(false);
                                if (titleDraft.trim() && titleDraft !== task.title) {
                                    updateMutation.mutate({ title: titleDraft.trim() });
                                } else {
                                    setTitleDraft(task.title);
                                }
                            }}
                            onKeyDown={e => {
                                if (e.key === "Enter") e.currentTarget.blur();
                                if (e.key === "Escape") { setTitleDraft(task.title); setEditingTitle(false); }
                            }}
                            className="bg-background w-full rounded border px-1 text-sm font-medium outline-none focus:ring-1 focus:ring-ring"
                        />
                    ) : (
                        <button type="button" onClick={() => setExpanded(e => !e)} className="text-left">
                            <span
                                onDoubleClick={e => { e.stopPropagation(); setEditingTitle(true); }}
                                className={`font-medium ${task.status === "done" ? "line-through text-muted-foreground" : ""}`}
                            >
                                {task.title}
                            </span>
                        </button>
                    )}
                    {task.description && !expanded && (
                        <div className="mt-1 line-clamp-1 text-xs text-muted-foreground">{task.description}</div>
                    )}
                </div>

                <button type="button" onClick={() => setExpanded(e => !e)} className="text-muted-foreground" aria-label="Toggle details">
                    {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                </button>
                <button
                    type="button"
                    onClick={() => deleteMutation.mutate()}
                    className="text-muted-foreground hover:text-destructive"
                    aria-label="Delete task"
                >
                    <Trash2 className="size-3.5" />
                </button>
            </div>

            {expanded && (
                <div className="border-t px-3 py-3 space-y-3 text-xs">
                    {/* Description editor */}
                    <div>
                        <div className="mb-1 text-[10px] uppercase text-muted-foreground">Description</div>
                        <textarea
                            value={descDraft}
                            onChange={e => setDescDraft(e.target.value)}
                            onBlur={saveDescription}
                            placeholder="Add a description (markdown supported)"
                            rows={3}
                            className="bg-background focus:ring-ring w-full rounded border p-2 text-xs leading-relaxed outline-none focus:ring-1 resize-y"
                        />
                    </div>

                    {/* Acceptance criteria */}
                    {task.acceptanceCriteria.length > 0 && (
                        <div className="space-y-1">
                            <div className="text-[10px] uppercase text-muted-foreground">
                                Acceptance ({task.acceptanceCriteria.filter(c => c.done).length}/{task.acceptanceCriteria.length})
                            </div>
                            {task.acceptanceCriteria.map((c, i) => (
                                <label key={i} className="hover:bg-muted/40 -mx-1 flex cursor-pointer items-start gap-2 rounded px-1 py-0.5">
                                    <input
                                        type="checkbox"
                                        checked={c.done}
                                        onChange={e => toggleCriterion(i, e.target.checked)}
                                        className="mt-0.5"
                                    />
                                    <span className={c.done ? "line-through text-muted-foreground" : ""}>{c.text}</span>
                                </label>
                            ))}
                        </div>
                    )}

                    {/* Chunk links */}
                    {task.chunks.length > 0 && (
                        <div>
                            <div className="text-[10px] uppercase text-muted-foreground">Chunks</div>
                            <div className="mt-1 flex flex-wrap gap-1">
                                {task.chunks.map(c => (
                                    <Link
                                        key={c.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: c.chunkId }}
                                        className="hover:bg-muted bg-muted/50 inline-flex items-center gap-1 rounded px-2 py-0.5 text-[11px] transition-colors"
                                        title={c.chunkType ?? undefined}
                                    >
                                        <span className="text-muted-foreground">{c.relation}</span>
                                        <span>·</span>
                                        <span className="font-medium">{c.chunkTitle ?? c.chunkId.slice(0, 8)}</span>
                                    </Link>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
