import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Settings as SettingsIcon } from "lucide-react";
import { useCallback, useRef } from "react";
import { toast } from "sonner";

import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/settings")({
    component: SettingsPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

function useDebounce(fn: (...args: any[]) => void, delay: number) {
    const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    return useCallback(
        (...args: any[]) => {
            if (timerRef.current) clearTimeout(timerRef.current);
            timerRef.current = setTimeout(() => fn(...args), delay);
        },
        [fn, delay]
    );
}

// --- User Settings Tab ---

function UserSettingsTab() {
    const queryClient = useQueryClient();

    const { data: settings } = useQuery({
        queryKey: ["settings", "user"],
        queryFn: async () => unwrapEden(await api.api.settings.user.get()) as Record<string, unknown>
    });

    const mutation = useMutation({
        mutationFn: async ({ key, value }: { key: string; value: unknown }) => {
            return unwrapEden(await api.api.settings.user.patch({ key, value }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["settings", "user"] });
        },
        onError: () => {
            toast.error("Failed to save setting");
        }
    });

    const save = useCallback(
        (key: string, value: unknown) => {
            mutation.mutate({ key, value });
        },
        [mutation]
    );

    const debouncedSave = useDebounce(save, 500);

    const theme = (settings?.theme as string) ?? "system";
    const defaultView = (settings?.defaultView as string) ?? "list";
    const defaultSort = (settings?.defaultSort as string) ?? "newest";
    const notificationsEnabled = (settings?.notificationsEnabled as boolean) ?? true;
    const notificationPollInterval = (settings?.notificationPollInterval as number) ?? 30;

    return (
        <div className="space-y-6">
            <Card>
                <CardPanel className="space-y-6 p-6">
                    <div>
                        <Label className="mb-2 block text-sm font-medium">Theme</Label>
                        <div className="flex gap-3">
                            {(["light", "dark", "system"] as const).map(opt => (
                                <label key={opt} className="flex items-center gap-2 text-sm">
                                    <input
                                        type="radio"
                                        name="theme"
                                        value={opt}
                                        checked={theme === opt}
                                        onChange={() => save("theme", opt)}
                                    />
                                    {opt.charAt(0).toUpperCase() + opt.slice(1)}
                                </label>
                            ))}
                        </div>
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">Default View</Label>
                        <div className="flex gap-3">
                            {(["list", "kanban"] as const).map(opt => (
                                <label key={opt} className="flex items-center gap-2 text-sm">
                                    <input
                                        type="radio"
                                        name="defaultView"
                                        value={opt}
                                        checked={defaultView === opt}
                                        onChange={() => save("defaultView", opt)}
                                    />
                                    {opt.charAt(0).toUpperCase() + opt.slice(1)}
                                </label>
                            ))}
                        </div>
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">Default Sort</Label>
                        <select
                            value={defaultSort}
                            onChange={e => save("defaultSort", e.target.value)}
                            className="bg-background focus:ring-ring w-48 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        >
                            <option value="newest">Newest</option>
                            <option value="oldest">Oldest</option>
                            <option value="alpha">Alphabetical</option>
                            <option value="updated">Last Updated</option>
                        </select>
                    </div>

                    <div className="flex items-center justify-between">
                        <Label className="text-sm font-medium">Notifications Enabled</Label>
                        <Switch
                            checked={notificationsEnabled}
                            onCheckedChange={(checked: boolean) => save("notificationsEnabled", checked)}
                        />
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">
                            Notification Poll Interval (seconds)
                        </Label>
                        <Input
                            type="number"
                            min={5}
                            max={300}
                            defaultValue={notificationPollInterval}
                            className="w-32"
                            onChange={e => debouncedSave("notificationPollInterval", Number(e.target.value))}
                        />
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}

// --- Codebase Settings Tab ---

function CodebaseSettingsTab() {
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();

    const { data: settings } = useQuery({
        queryKey: ["settings", "codebase", codebaseId],
        queryFn: async () => {
            if (!codebaseId) return {};
            return unwrapEden(
                await api.api.settings.codebase.get({ query: { codebaseId } })
            ) as Record<string, unknown>;
        },
        enabled: !!codebaseId
    });

    const mutation = useMutation({
        mutationFn: async ({ key, value }: { key: string; value: unknown }) => {
            if (!codebaseId) throw new Error("No codebase");
            return unwrapEden(await api.api.settings.codebase.patch({ codebaseId, key, value }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["settings", "codebase", codebaseId] });
        },
        onError: () => {
            toast.error("Failed to save setting");
        }
    });

    const save = useCallback(
        (key: string, value: unknown) => {
            mutation.mutate({ key, value });
        },
        [mutation]
    );

    const debouncedSave = useDebounce(save, 500);

    if (!codebaseId) {
        return (
            <Card>
                <CardPanel className="p-6">
                    <p className="text-muted-foreground text-sm">Select a codebase to manage its settings.</p>
                </CardPanel>
            </Card>
        );
    }

    const defaultChunkType = (settings?.defaultChunkType as string) ?? "";
    const requireReviewForAi = (settings?.requireReviewForAi as boolean) ?? false;
    const autoEnrichOnCreate = (settings?.autoEnrichOnCreate as boolean) ?? false;
    const defaultTags = (settings?.defaultTags as string[]) ?? [];
    const templateId = (settings?.templateId as string) ?? "";

    return (
        <div className="space-y-6">
            <Card>
                <CardPanel className="space-y-6 p-6">
                    <div>
                        <Label className="mb-2 block text-sm font-medium">Default Chunk Type</Label>
                        <Input
                            defaultValue={defaultChunkType}
                            placeholder="e.g. concept, decision, pattern"
                            className="w-64"
                            onChange={e => debouncedSave("defaultChunkType", e.target.value)}
                        />
                    </div>

                    <div className="flex items-center justify-between">
                        <Label className="text-sm font-medium">Require Review for AI Content</Label>
                        <Switch
                            checked={requireReviewForAi}
                            onCheckedChange={(checked: boolean) => save("requireReviewForAi", checked)}
                        />
                    </div>

                    <div className="flex items-center justify-between">
                        <Label className="text-sm font-medium">Auto-Enrich on Create</Label>
                        <Switch
                            checked={autoEnrichOnCreate}
                            onCheckedChange={(checked: boolean) => save("autoEnrichOnCreate", checked)}
                        />
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">Default Tags</Label>
                        <Input
                            defaultValue={defaultTags.join(", ")}
                            placeholder="tag1, tag2, tag3"
                            className="w-64"
                            onChange={e =>
                                debouncedSave(
                                    "defaultTags",
                                    e.target.value
                                        .split(",")
                                        .map(t => t.trim())
                                        .filter(Boolean)
                                )
                            }
                        />
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">Default Template ID</Label>
                        <Input
                            defaultValue={templateId}
                            placeholder="Template ID (optional)"
                            className="w-64"
                            onChange={e =>
                                debouncedSave("templateId", e.target.value || null)
                            }
                        />
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}

// --- Instance Settings Tab ---

function InstanceSettingsTab() {
    const queryClient = useQueryClient();

    const { data: settings } = useQuery({
        queryKey: ["settings", "instance"],
        queryFn: async () => unwrapEden(await api.api.settings.instance.get()) as Record<string, unknown>
    });

    const mutation = useMutation({
        mutationFn: async ({ key, value }: { key: string; value: unknown }) => {
            return unwrapEden(await api.api.settings.instance.patch({ key, value }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["settings", "instance"] });
            queryClient.invalidateQueries({ queryKey: ["feature-flags"] });
        },
        onError: () => {
            toast.error("Failed to save setting");
        }
    });

    const save = useCallback(
        (key: string, value: unknown) => {
            mutation.mutate({ key, value });
        },
        [mutation]
    );

    const debouncedSave = useDebounce(save, 500);

    const aiEnabled = (settings?.aiEnabled as boolean) ?? true;
    const enrichmentEnabled = (settings?.enrichmentEnabled as boolean) ?? true;
    const semanticSearchEnabled = (settings?.semanticSearchEnabled as boolean) ?? true;
    const aiSuggestionsEnabled = (settings?.aiSuggestionsEnabled as boolean) ?? true;
    const vocabularySuggestEnabled = (settings?.vocabularySuggestEnabled as boolean) ?? true;
    const ollamaUrl = (settings?.ollamaUrl as string) ?? "http://localhost:11434";
    const registrationEnabled = (settings?.registrationEnabled as boolean) ?? true;
    const maxChunksPerCodebase = (settings?.maxChunksPerCodebase as number) ?? 10000;

    return (
        <div className="space-y-6">
            <Card>
                <CardPanel className="space-y-6 p-6">
                    <h3 className="text-sm font-semibold tracking-tight">AI Features</h3>

                    <div className="flex items-center justify-between">
                        <div>
                            <Label className="text-sm font-medium">Enable AI Features</Label>
                            <p className="text-muted-foreground text-xs">Master toggle for all AI features</p>
                        </div>
                        <Switch
                            checked={aiEnabled}
                            onCheckedChange={(checked: boolean) => save("aiEnabled", checked)}
                        />
                    </div>

                    <div className={`space-y-4 ${!aiEnabled ? "pointer-events-none opacity-50" : ""}`}>
                        <div className="flex items-center justify-between">
                            <Label className="text-sm font-medium">Enrichment</Label>
                            <Switch
                                checked={enrichmentEnabled}
                                onCheckedChange={(checked: boolean) => save("enrichmentEnabled", checked)}
                            />
                        </div>

                        <div className="flex items-center justify-between">
                            <Label className="text-sm font-medium">Semantic Search</Label>
                            <Switch
                                checked={semanticSearchEnabled}
                                onCheckedChange={(checked: boolean) => save("semanticSearchEnabled", checked)}
                            />
                        </div>

                        <div className="flex items-center justify-between">
                            <Label className="text-sm font-medium">AI Suggestions</Label>
                            <Switch
                                checked={aiSuggestionsEnabled}
                                onCheckedChange={(checked: boolean) => save("aiSuggestionsEnabled", checked)}
                            />
                        </div>

                        <div className="flex items-center justify-between">
                            <Label className="text-sm font-medium">Vocabulary Suggest</Label>
                            <Switch
                                checked={vocabularySuggestEnabled}
                                onCheckedChange={(checked: boolean) => save("vocabularySuggestEnabled", checked)}
                            />
                        </div>
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">Ollama URL</Label>
                        <Input
                            defaultValue={ollamaUrl}
                            placeholder="http://localhost:11434"
                            className="w-80"
                            onChange={e => debouncedSave("ollamaUrl", e.target.value)}
                        />
                    </div>
                </CardPanel>
            </Card>

            <Card>
                <CardPanel className="space-y-6 p-6">
                    <h3 className="text-sm font-semibold tracking-tight">General</h3>

                    <div className="flex items-center justify-between">
                        <Label className="text-sm font-medium">Registration Enabled</Label>
                        <Switch
                            checked={registrationEnabled}
                            onCheckedChange={(checked: boolean) => save("registrationEnabled", checked)}
                        />
                    </div>

                    <div>
                        <Label className="mb-2 block text-sm font-medium">Max Chunks Per Codebase</Label>
                        <Input
                            type="number"
                            min={100}
                            max={100000}
                            defaultValue={maxChunksPerCodebase}
                            className="w-32"
                            onChange={e => debouncedSave("maxChunksPerCodebase", Number(e.target.value))}
                        />
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}

// --- Main Settings Page ---

function SettingsPage() {
    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center gap-2">
                <SettingsIcon className="size-5" />
                <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
            </div>

            <Tabs defaultValue={0}>
                <TabsList>
                    <TabsTab value={0}>Preferences</TabsTab>
                    <TabsTab value={1}>Codebase</TabsTab>
                    <TabsTab value={2}>Instance</TabsTab>
                </TabsList>

                <TabsContent value={0} className="mt-4">
                    <UserSettingsTab />
                </TabsContent>

                <TabsContent value={1} className="mt-4">
                    <CodebaseSettingsTab />
                </TabsContent>

                <TabsContent value={2} className="mt-4">
                    <InstanceSettingsTab />
                </TabsContent>
            </Tabs>
        </div>
    );
}
