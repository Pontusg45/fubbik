import { useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import type { Tag, TagType, SortMode } from "./tag-types";

export function useTagsData() {
    const queryClient = useQueryClient();
    const [search, setSearch] = useState("");
    const [sortMode, setSortMode] = useState<SortMode>("alpha");
    const [unusedOnly, setUnusedOnly] = useState(false);

    // Rename state — inline on the pill
    const [renamingId, setRenamingId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState("");

    // Tag type form state
    const [showTagTypeForm, setShowTagTypeForm] = useState(false);
    const [editingTagType, setEditingTagType] = useState<TagType | null>(null);
    const [ttName, setTtName] = useState("");
    const [ttColor, setTtColor] = useState("#8b5cf6");

    const tagsQuery = useApiQuery<Tag[]>({
        queryKey: ["tags"],
        queryFn: () => api.api.tags.get(),
        fallback: [],
    });

    const tagTypesQuery = useApiQuery<TagType[]>({
        queryKey: ["tag-types"],
        queryFn: () => api.api["tag-types"].get(),
        fallback: [],
    });

    // --- Mutations ---

    const createTagMutation = useMutation({
        mutationFn: async (body: { name: string; tagTypeId?: string }) =>
            unwrapEden(await api.api.tags.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success("Tag created");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to create tag";
            toast.error(msg);
        }
    });

    const renameTagMutation = useMutation({
        mutationFn: async ({ id, name }: { id: string; name: string }) =>
            unwrapEden(await api.api.tags({ id }).patch({ name })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            setRenamingId(null);
            setRenameValue("");
            toast.success("Tag renamed");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to rename tag";
            toast.error(msg);
        }
    });

    const assignTypeMutation = useMutation({
        mutationFn: async ({ id, tagTypeId }: { id: string; tagTypeId: string | null }) =>
            unwrapEden(await api.api.tags({ id }).patch({ tagTypeId })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success("Tag type updated");
        },
        onError: () => toast.error("Failed to update tag")
    });

    const mergeMutation = useMutation({
        mutationFn: async ({ sourceId, targetId }: { sourceId: string; targetId: string }) =>
            unwrapEden(await api.api.tags.merge.post({ sourceId, targetId })) as { targetId: string; chunkCount: number },
        onSuccess: result => {
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success(`Merged — target now has ${result.chunkCount} chunks`);
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to merge tags";
            toast.error(msg);
        }
    });

    const deleteTagMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api.tags({ id }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success("Tag deleted");
        },
        onError: () => toast.error("Failed to delete tag")
    });

    const createTagTypeMutation = useMutation({
        mutationFn: async (body: { name: string; color: string }) =>
            unwrapEden(await api.api["tag-types"].post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tag-types"] });
            resetTagTypeForm();
            toast.success("Tag type created");
        },
        onError: () => toast.error("Failed to create tag type")
    });

    const updateTagTypeMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: { name?: string; color?: string } }) =>
            unwrapEden(await api.api["tag-types"]({ id }).patch(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tag-types"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            resetTagTypeForm();
            toast.success("Tag type updated");
        },
        onError: () => toast.error("Failed to update tag type")
    });

    const deleteTagTypeMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api["tag-types"]({ id }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tag-types"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success("Tag type deleted");
        },
        onError: () => toast.error("Failed to delete tag type")
    });

    // --- Tag type form helpers ---

    function resetTagTypeForm() {
        setShowTagTypeForm(false);
        setEditingTagType(null);
        setTtName("");
        setTtColor("#8b5cf6");
    }

    function startEditTagType(tt: TagType) {
        setEditingTagType(tt);
        setTtName(tt.name);
        setTtColor(tt.color);
        setShowTagTypeForm(true);
    }

    function handleTagTypeSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!ttName.trim()) return;
        if (editingTagType) {
            updateTagTypeMutation.mutate({ id: editingTagType.id, body: { name: ttName.trim(), color: ttColor } });
        } else {
            createTagTypeMutation.mutate({ name: ttName.trim(), color: ttColor });
        }
    }

    // --- Tag rename helpers ---

    function startRename(tag: Tag) {
        setRenamingId(tag.id);
        setRenameValue(tag.name);
    }

    function commitRename(tag: Tag) {
        const next = renameValue.trim();
        if (!next || next === tag.name) {
            setRenamingId(null);
            setRenameValue("");
            return;
        }
        renameTagMutation.mutate({ id: tag.id, name: next });
    }

    // --- Derived data ---

    const tags = Array.isArray(tagsQuery.data) ? tagsQuery.data : [];
    const tagTypes = Array.isArray(tagTypesQuery.data) ? tagTypesQuery.data : [];

    const filteredTags = useMemo(() => {
        const q = search.trim().toLowerCase();
        let list = tags;
        if (unusedOnly) list = list.filter(t => t.chunkCount === 0);
        if (!q) return list;
        return list.filter(
            t =>
                t.name.toLowerCase().includes(q) ||
                (t.tagTypeName?.toLowerCase().includes(q) ?? false)
        );
    }, [tags, search, unusedOnly]);

    const sortedGroups = useMemo(() => {
        const grouped = new Map<string, { tagType: TagType | null; tags: Tag[] }>();
        for (const t of filteredTags) {
            const key = t.tagTypeId ?? "__none__";
            if (!grouped.has(key)) {
                grouped.set(key, {
                    tagType: t.tagTypeId
                        ? { id: t.tagTypeId, name: t.tagTypeName ?? "Unknown", color: t.tagTypeColor ?? "#888" }
                        : null,
                    tags: []
                });
            }
            grouped.get(key)!.tags.push(t);
        }
        const cmp = sortMode === "usage"
            ? (a: Tag, b: Tag) => b.chunkCount - a.chunkCount || a.name.localeCompare(b.name)
            : (a: Tag, b: Tag) => a.name.localeCompare(b.name);
        for (const group of grouped.values()) group.tags.sort(cmp);
        return [...grouped.entries()].sort((a, b) => {
            if (!a[1].tagType) return 1;
            if (!b[1].tagType) return -1;
            return a[1].tagType.name.localeCompare(b[1].tagType.name);
        });
    }, [filteredTags, sortMode]);

    const unusedCount = useMemo(() => tags.filter(t => t.chunkCount === 0).length, [tags]);

    return {
        // Queries
        tagsQuery,
        tagTypesQuery,
        // Derived data
        tags,
        tagTypes,
        filteredTags,
        sortedGroups,
        unusedCount,
        // Search / filter state
        search,
        setSearch,
        sortMode,
        setSortMode,
        unusedOnly,
        setUnusedOnly,
        // Rename state
        renamingId,
        renameValue,
        setRenameValue,
        startRename,
        commitRename,
        cancelRename: () => { setRenamingId(null); setRenameValue(""); },
        // Tag type form state
        showTagTypeForm,
        setShowTagTypeForm,
        editingTagType,
        ttName,
        setTtName,
        ttColor,
        setTtColor,
        resetTagTypeForm,
        startEditTagType,
        handleTagTypeSubmit,
        // Mutations
        createTagMutation,
        renameTagMutation,
        assignTypeMutation,
        mergeMutation,
        deleteTagMutation,
        createTagTypeMutation,
        updateTagTypeMutation,
        deleteTagTypeMutation,
    };
}
