import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import React, { useCallback, useMemo } from "react";
import { toast } from "sonner";

import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { useRecentChunks } from "@/features/chunks/use-recent-chunks";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import {
    buildActionItems,
    buildChunkQuickOpenItems,
    buildChunkSearchItems,
    buildCodebaseItems,
    buildFederatedItems,
    buildPageItems,
    buildPlanItems,
    buildRecentChunkItems,
    buildRecentPageItems,
    buildRequirementItems,
    buildTagItems,
} from "./command-items";
import type { CommandItem, RecentPage } from "./command-types";

interface UseCommandSearchOptions {
    open: boolean;
    query: string;
    subMode: string | null;
    recentPages: RecentPage[];
    close: () => void;
    setSubMode: (mode: string | null) => void;
    setQuery: (q: string) => void;
    setSelectedIndex: (i: number) => void;
}

export function useCommandSearch({
    open,
    query,
    subMode,
    recentPages,
    close,
    setSubMode,
    setQuery,
    setSelectedIndex,
}: UseCommandSearchOptions): { items: CommandItem[]; isLoading: boolean } {
    const navigate = useNavigate();
    const { recentIds } = useRecentChunks();
    const { setCodebaseId } = useActiveCodebase();

    const debouncedQuery = useDebouncedValue(query, 200);
    const isChunkMode = subMode === "chunks";
    const isTagSearch = query.startsWith("#");
    const tagQuery = isTagSearch ? query.slice(1).toLowerCase() : "";
    const isFederatedSearch = query.startsWith("*");
    const federatedQuery = isFederatedSearch ? query.slice(1).trim() : "";
    const debouncedFederatedQuery = useDebouncedValue(federatedQuery, 200);

    const quickNoteMutation = useMutation({
        mutationFn: async (title: string) => {
            const result = unwrapEden(
                await api.api.chunks.post({
                    title,
                    content: "",
                    type: "note",
                })
            );
            return result;
        },
        onSuccess: (data) => {
            const chunk = data as { id: string };
            toast.success(`Created "${query.trim()}"`, {
                action: {
                    label: "Edit",
                    onClick: () => navigate({ to: "/chunks/$chunkId/edit", params: { chunkId: chunk.id } }),
                },
            });
            close();
        },
        onError: () => {
            toast.error("Failed to create note");
        },
    });

    // Search chunks via API
    const chunkSearch = useQuery({
        queryKey: ["command-palette-chunks", debouncedQuery],
        queryFn: async () => {
            if (!debouncedQuery.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: { search: debouncedQuery, limit: "5" },
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: open && debouncedQuery.length > 1 && !isTagSearch && !isFederatedSearch,
    });

    // Federated search (across all codebases) when query starts with *
    const federatedSearch = useQuery({
        queryKey: ["command-palette-federated", debouncedFederatedQuery],
        queryFn: async () => {
            if (!debouncedFederatedQuery.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.chunks.search.federated.get({
                        query: { search: debouncedFederatedQuery, limit: "8" },
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: open && isFederatedSearch && debouncedFederatedQuery.length > 0,
    });

    // Search tags via API when query starts with #
    const tagSearch = useQuery({
        queryKey: ["command-palette-tags"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.tags.get()) as Array<{
                    id: string;
                    name: string;
                    tagTypeName: string | null;
                    tagTypeColor: string | null;
                }>;
            } catch {
                return [];
            }
        },
        enabled: open && isTagSearch,
    });

    // Search requirements via API
    const requirementsSearch = useQuery({
        queryKey: ["command-palette-requirements", debouncedQuery],
        queryFn: async () => {
            if (!debouncedQuery.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.requirements.get({
                        query: { search: debouncedQuery, limit: "5" },
                    })
                ) as { requirements: Array<{ id: string; title: string; status: string; priority: string }>; total: number };
            } catch {
                return null;
            }
        },
        enabled: open && debouncedQuery.length > 1 && !isTagSearch && !isFederatedSearch && subMode === null,
    });

    // Fetch all plans and filter client-side (API doesn't support search param)
    const plansSearch = useQuery({
        queryKey: ["command-palette-plans"],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.plans.get({ query: {} })
                ) as Array<{ id: string; title: string; status: string }>;
            } catch {
                return [];
            }
        },
        enabled: open && debouncedQuery.length > 1 && !isTagSearch && !isFederatedSearch && subMode === null,
        staleTime: 30_000,
    });

    // Fetch codebases for Switch Codebase sub-mode
    const codebasesQuery = useQuery({
        queryKey: ["command-palette-codebases"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.codebases.get()) as Array<{
                    id: string;
                    name: string;
                    remoteUrl: string | null;
                }>;
            } catch {
                return [];
            }
        },
        enabled: open && subMode === "codebase",
        staleTime: 60_000,
    });

    // Fetch all chunks for the chunk quick-open (Ctrl+O) mode
    const allChunksQuery = useQuery({
        queryKey: ["command-palette-all-chunks"],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.chunks.get({ query: { limit: "500" } })
                ) as { chunks: Array<{ id: string; title: string; type: string }> };
            } catch {
                return { chunks: [] };
            }
        },
        enabled: open && isChunkMode,
        staleTime: 300_000,
    });

    // Fetch recent chunk details
    const recentChunksQuery = useQuery({
        queryKey: ["command-palette-recent", recentIds],
        queryFn: async () => {
            if (recentIds.length === 0) return [];
            try {
                const results = await Promise.all(
                    recentIds.slice(0, 5).map(async (id) => {
                        try {
                            const data = unwrapEden(
                                await api.api.chunks({ id }).get()
                            );
                            return data;
                        } catch {
                            return null;
                        }
                    })
                );
                return results.filter(Boolean);
            } catch {
                return [];
            }
        },
        enabled: open && !query.trim() && recentIds.length > 0,
    });

    const isLoading =
        chunkSearch.isFetching ||
        federatedSearch.isFetching ||
        tagSearch.isFetching ||
        requirementsSearch.isFetching ||
        plansSearch.isFetching ||
        codebasesQuery.isFetching ||
        allChunksQuery.isFetching;

    const handleChunkNavigate = useCallback(
        (id: string) => {
            navigate({ to: "/chunks/$chunkId", params: { chunkId: id } });
            close();
        },
        [navigate, close]
    );

    const handleRequirementNavigate = useCallback(
        (id: string) => {
            navigate({ to: "/requirements/$requirementId", params: { requirementId: id } });
            close();
        },
        [navigate, close]
    );

    const handlePlanNavigate = useCallback(
        (id: string) => {
            navigate({ to: "/plans/$planId", params: { planId: id } });
            close();
        },
        [navigate, close]
    );

    const items = useMemo(() => {
        const lowerQuery = query.toLowerCase();
        const result: CommandItem[] = [];

        // Sub-mode: codebase switcher
        if (subMode === "codebase") {
            return buildCodebaseItems(codebasesQuery.data ?? [], lowerQuery, (id) => {
                setCodebaseId(id);
                close();
            });
        }

        // Sub-mode: chunk quick-open (Ctrl+O)
        if (isChunkMode) {
            return buildChunkQuickOpenItems(
                allChunksQuery.data?.chunks ?? [],
                lowerQuery,
                handleChunkNavigate
            );
        }

        // Federated search mode: when query starts with *
        if (isFederatedSearch) {
            const chunks = (federatedSearch.data?.chunks ?? []) as Array<Record<string, unknown>>;
            return buildFederatedItems(chunks, handleChunkNavigate);
        }

        // Tag search mode: when query starts with #
        if (isTagSearch) {
            return buildTagItems(tagSearch.data ?? [], tagQuery, (name) => {
                navigate({ to: "/chunks", search: { tags: name } });
                close();
            });
        }

        // Recent items (shown when query is empty)
        if (!query.trim()) {
            result.push(
                ...buildRecentPageItems(recentPages, (path) => {
                    navigate({ to: path });
                    close();
                })
            );
            result.push(
                ...buildRecentChunkItems(recentChunksQuery.data ?? [], handleChunkNavigate)
            );
        }

        // Pages
        result.push(
            ...buildPageItems(lowerQuery, (path) => {
                navigate({ to: path });
                close();
            })
        );

        // Chunks from API search
        result.push(
            ...buildChunkSearchItems(chunkSearch.data?.chunks ?? [], handleChunkNavigate)
        );

        // Requirements and Plans (only when query is long enough)
        if (debouncedQuery.length > 1) {
            result.push(
                ...buildRequirementItems(
                    requirementsSearch.data?.requirements ?? [],
                    handleRequirementNavigate
                )
            );

            const allPlans = Array.isArray(plansSearch.data) ? plansSearch.data : [];
            result.push(...buildPlanItems(allPlans, lowerQuery, handlePlanNavigate));
        }

        // Actions
        result.push(
            ...buildActionItems(lowerQuery, (action) => {
                if (action.subMode) {
                    setSubMode(action.subMode);
                    setQuery("");
                    setSelectedIndex(0);
                } else {
                    navigate({ to: action.path, search: action.search ?? {} });
                    close();
                }
            })
        );

        // Quick note creation
        if (query.trim().length > 0 && !isTagSearch && !isFederatedSearch && !subMode) {
            result.push({
                id: "quick-note",
                title: `Quick note: "${query.trim()}"`,
                group: "Actions",
                icon: React.createElement(Plus, { className: "size-4" }),
                badge: "Create",
                onSelect: () => quickNoteMutation.mutate(query.trim()),
            });
        }

        return result;
    }, [
        query,
        debouncedQuery,
        subMode,
        isChunkMode,
        isTagSearch,
        isFederatedSearch,
        tagQuery,
        tagSearch.data,
        federatedSearch.data,
        recentPages,
        recentChunksQuery.data,
        chunkSearch.data,
        requirementsSearch.data,
        plansSearch.data,
        codebasesQuery.data,
        allChunksQuery.data,
        navigate,
        close,
        setCodebaseId,
        quickNoteMutation,
        handleChunkNavigate,
        handleRequirementNavigate,
        handlePlanNavigate,
        setSubMode,
        setQuery,
        setSelectedIndex,
    ]);

    return { items, isLoading };
}
