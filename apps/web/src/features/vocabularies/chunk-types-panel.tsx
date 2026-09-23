import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Check, Lock, Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { CHUNK_TYPE_ICONS, resolveChunkTypeIcon, useChunkTypes, type ChunkTypeMeta } from "@/features/vocabularies/use-vocabularies";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { extractMessage } from "./mutation-error";

// ── Chunk types ──────────────────────────────────────────────────────────

export function ChunkTypesPanel() {
    const qc = useQueryClient();
    const { data, isLoading } = useChunkTypes();
    const [adding, setAdding] = useState(false);

    const createMutation = useMutation({
        mutationFn: async (body: ChunkTypeCreateBody) => unwrapEden(await api.api["chunk-types"].post(body)),
        onSuccess: () => {
            qc.invalidateQueries({ queryKey: ["chunk-types"] });
            toast.success("Chunk type added");
            setAdding(false);
        },
        onError: (err: unknown) => toast.error(extractMessage(err))
    });

    const updateMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: Partial<ChunkTypeCreateBody> }) =>
            unwrapEden(await api.api["chunk-types"]({ id }).patch(body)),
        onSuccess: () => {
            qc.invalidateQueries({ queryKey: ["chunk-types"] });
            toast.success("Chunk type updated");
        },
        onError: (err: unknown) => toast.error(extractMessage(err))
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api["chunk-types"]({ id }).delete()),
        onSuccess: () => {
            qc.invalidateQueries({ queryKey: ["chunk-types"] });
            toast.success("Chunk type deleted");
        },
        onError: (err: unknown) => toast.error(extractMessage(err))
    });

    return (
        <section>
            <div className="mb-3 flex items-center justify-between">
                <div>
                    <h2 className="text-lg font-semibold">Chunk types</h2>
                    <p className="text-muted-foreground text-xs">
                        Used everywhere chunks are listed: graph icons, filter dropdowns, chunk cards.
                    </p>
                </div>
                {!adding && (
                    <Button size="sm" onClick={() => setAdding(true)}>
                        <Plus className="size-3.5" />
                        Add type
                    </Button>
                )}
            </div>

            {adding && (
                <ChunkTypeForm
                    mode="create"
                    onCancel={() => setAdding(false)}
                    onSubmit={body => createMutation.mutate(body)}
                    submitting={createMutation.isPending}
                />
            )}

            <Card>
                {isLoading ? (
                    <CardPanel className="text-muted-foreground p-4 text-sm">Loading...</CardPanel>
                ) : !data || data.length === 0 ? (
                    <CardPanel className="text-muted-foreground p-4 text-sm">No chunk types yet.</CardPanel>
                ) : (
                    data.map((meta, i) => (
                        <div key={meta.id}>
                            {i > 0 && <Separator />}
                            <ChunkTypeRow
                                meta={meta}
                                onUpdate={body => updateMutation.mutate({ id: meta.id, body })}
                                onDelete={() => deleteMutation.mutate(meta.id)}
                                busy={
                                    (updateMutation.isPending && updateMutation.variables?.id === meta.id) ||
                                    (deleteMutation.isPending && deleteMutation.variables === meta.id)
                                }
                            />
                        </div>
                    ))
                )}
            </Card>
        </section>
    );
}

function ChunkTypeRow({
    meta,
    onUpdate,
    onDelete,
    busy
}: {
    meta: ChunkTypeMeta;
    onUpdate: (body: Partial<ChunkTypeCreateBody>) => void;
    onDelete: () => void;
    busy: boolean;
}) {
    const [editing, setEditing] = useState(false);
    const Icon = resolveChunkTypeIcon(meta.icon);

    if (editing) {
        return (
            <CardPanel className="p-4">
                <ChunkTypeForm
                    mode="edit"
                    initial={meta}
                    onCancel={() => setEditing(false)}
                    onSubmit={body => {
                        onUpdate(body);
                        setEditing(false);
                    }}
                    submitting={busy}
                />
            </CardPanel>
        );
    }

    return (
        <CardPanel className="flex items-center gap-3 p-4">
            <div className="flex size-8 shrink-0 items-center justify-center rounded-md border" style={{ borderColor: meta.color }}>
                <Icon className="size-4" style={{ color: meta.color }} />
            </div>
            <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                    <span className="font-medium">{meta.label}</span>
                    <span className="text-muted-foreground font-mono text-xs">{meta.id}</span>
                    {meta.builtIn && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            <Lock className="size-2.5" />
                            built-in
                        </Badge>
                    )}
                </div>
                {meta.description && <p className="text-muted-foreground mt-0.5 text-xs">{meta.description}</p>}
                {meta.examples.length > 0 && (
                    <p className="text-muted-foreground mt-0.5 text-xs italic">e.g., {meta.examples.join(", ")}</p>
                )}
            </div>
            {!meta.builtIn && (
                <div className="flex gap-1">
                    <Button variant="outline" size="xs" onClick={() => setEditing(true)}>
                        <Pencil className="size-3" />
                    </Button>
                    <Button variant="outline" size="xs" onClick={onDelete} disabled={busy}>
                        <Trash2 className="size-3" />
                    </Button>
                </div>
            )}
        </CardPanel>
    );
}

interface ChunkTypeCreateBody {
    id: string;
    label: string;
    description?: string | null;
    icon?: string | null;
    color?: string;
    examples?: string[];
}

function ChunkTypeForm({
    mode,
    initial,
    onCancel,
    onSubmit,
    submitting
}: {
    mode: "create" | "edit";
    initial?: ChunkTypeMeta;
    onCancel: () => void;
    onSubmit: (body: ChunkTypeCreateBody) => void;
    submitting: boolean;
}) {
    const [id, setId] = useState(initial?.id ?? "");
    const [label, setLabel] = useState(initial?.label ?? "");
    const [description, setDescription] = useState(initial?.description ?? "");
    const [icon, setIcon] = useState(initial?.icon ?? "FileText");
    const [color, setColor] = useState(initial?.color ?? "#8b5cf6");
    const [examplesText, setExamplesText] = useState((initial?.examples ?? []).join(", "));

    return (
        <Card className="mb-4">
            <CardPanel className="space-y-3 p-4">
                <div className="grid grid-cols-2 gap-3">
                    <div>
                        <Label className="text-xs">Slug (id)</Label>
                        <Input
                            value={id}
                            onChange={e => setId(e.target.value)}
                            placeholder="runbook"
                            disabled={mode === "edit"}
                            className="font-mono"
                        />
                    </div>
                    <div>
                        <Label className="text-xs">Label</Label>
                        <Input value={label} onChange={e => setLabel(e.target.value)} placeholder="Runbook" />
                    </div>
                </div>
                <div>
                    <Label className="text-xs">Description</Label>
                    <Input
                        value={description ?? ""}
                        onChange={e => setDescription(e.target.value)}
                        placeholder="Operational procedure — how to respond to an incident"
                    />
                </div>
                <div className="grid grid-cols-3 gap-3">
                    <div>
                        <Label className="text-xs">Icon</Label>
                        <select
                            value={icon}
                            onChange={e => setIcon(e.target.value)}
                            className="bg-background w-full rounded-md border px-2 py-1.5 text-xs"
                        >
                            {Object.keys(CHUNK_TYPE_ICONS).map(n => (
                                <option key={n} value={n}>
                                    {n}
                                </option>
                            ))}
                        </select>
                    </div>
                    <div>
                        <Label className="text-xs">Color</Label>
                        <Input type="color" value={color} onChange={e => setColor(e.target.value)} className="h-9 p-1" />
                    </div>
                    <div>
                        <Label className="text-xs">Examples (comma-separated)</Label>
                        <Input
                            value={examplesText}
                            onChange={e => setExamplesText(e.target.value)}
                            placeholder="Incident response, Deploy rollback"
                        />
                    </div>
                </div>
                <div className="flex items-center justify-end gap-2">
                    <Button variant="ghost" size="sm" onClick={onCancel}>
                        Cancel
                    </Button>
                    <Button
                        size="sm"
                        disabled={submitting || !id.trim() || !label.trim()}
                        onClick={() =>
                            onSubmit({
                                id: id.trim(),
                                label: label.trim(),
                                description: description.trim() || null,
                                icon: icon || null,
                                color,
                                examples: examplesText
                                    .split(",")
                                    .map(s => s.trim())
                                    .filter(Boolean)
                            })
                        }
                    >
                        <Check className="size-3.5" />
                        {mode === "create" ? "Create" : "Save"}
                    </Button>
                </div>
            </CardPanel>
        </Card>
    );
}
