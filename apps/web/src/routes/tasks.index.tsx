import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Check, ClipboardCheck, Plus } from "lucide-react";
import { useState, type FormEvent } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { useActiveSpace } from "@/features/spaces/use-active-space";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/tasks/")({ component: TasksPage });

type TaskView = "active" | "completed";
type Priority = "low" | "normal" | "high" | "urgent";

const PRIORITY_LABEL: Record<Priority, string> = { low: "Low", normal: "Normal", high: "High", urgent: "Urgent" };

function TasksPage() {
    const queryClient = useQueryClient();
    const { spaceId } = useActiveSpace();
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [priority, setPriority] = useState<Priority>("normal");
    const [view, setView] = useState<TaskView>("active");

    const tasksQuery = useQuery({
        queryKey: ["queue-tasks", view],
        queryFn: async () => unwrapEden(await api.api.tasks.get({ query: { status: view === "active" ? "in_progress" : "completed" } }))
    });

    const createTask = useMutation({
        mutationFn: async () =>
            unwrapEden(
                await api.api.tasks.post({
                    title: title.trim(),
                    description: description.trim() || undefined,
                    priority,
                    spaceId: spaceId && spaceId !== "global" ? spaceId : undefined
                })
            ),
        onSuccess: () => {
            setTitle("");
            setDescription("");
            setPriority("normal");
            queryClient.invalidateQueries({ queryKey: ["queue-tasks"] });
            queryClient.invalidateQueries({ queryKey: ["plans"] });
            toast.success("Task created");
        },
        onError: () => toast.error("Failed to create task")
    });

    const completeTask = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api.tasks({ id }).complete.post({})),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["queue-tasks"] });
            queryClient.invalidateQueries({ queryKey: ["plans"] });
            toast.success("Task completed");
        },
        onError: () => toast.error("Failed to complete task")
    });

    const tasks = (tasksQuery.data ?? []).filter(task => !spaceId || (spaceId === "global" ? !task.spaceId : task.spaceId === spaceId));

    function submit(event: FormEvent<HTMLFormElement>) {
        event.preventDefault();
        if (title.trim() && !createTask.isPending) createTask.mutate();
    }

    return (
        <PageContainer>
            <PageHeader
                icon={ClipboardCheck}
                title="Tasks"
                description="Capture standalone work and keep it moving. Tasks inside larger plans live on their plan pages."
                actions={
                    <Button variant="outline" render={<Link to="/plans" />}>
                        View plans
                    </Button>
                }
            />

            <Card className="mb-6">
                <CardPanel className="p-4">
                    <form onSubmit={submit} className="space-y-3">
                        <label htmlFor="task-title" className="text-sm font-medium">
                            New task
                        </label>
                        <div className="flex gap-2">
                            <Input
                                id="task-title"
                                aria-label="Task title"
                                placeholder="What needs to be done?"
                                value={title}
                                onChange={event => setTitle(event.target.value)}
                            />
                            <Button type="submit" disabled={!title.trim() || createTask.isPending}>
                                <Plus className="size-4" /> Add task
                            </Button>
                        </div>
                        <Input
                            aria-label="Task description"
                            placeholder="Description (optional)"
                            value={description}
                            onChange={event => setDescription(event.target.value)}
                        />
                        <label htmlFor="task-priority" className="text-sm font-medium">
                            Priority
                        </label>
                        <select
                            id="task-priority"
                            className="bg-background rounded-md border px-3 py-1.5 text-sm"
                            value={priority}
                            onChange={event => setPriority(event.target.value as Priority)}
                        >
                            {(Object.keys(PRIORITY_LABEL) as Priority[]).map(value => (
                                <option key={value} value={value}>
                                    {PRIORITY_LABEL[value]}
                                </option>
                            ))}
                        </select>
                    </form>
                </CardPanel>
            </Card>

            <div className="mb-4 flex gap-2" aria-label="Task status">
                <Button
                    variant={view === "active" ? "default" : "outline"}
                    aria-pressed={view === "active"}
                    onClick={() => setView("active")}
                >
                    Active
                </Button>
                <Button
                    variant={view === "completed" ? "default" : "outline"}
                    aria-pressed={view === "completed"}
                    onClick={() => setView("completed")}
                >
                    Completed
                </Button>
            </div>

            {tasksQuery.isPending ? (
                <PageLoading count={3} />
            ) : tasksQuery.isError ? (
                <PageEmpty
                    variant="error"
                    error={tasksQuery.error}
                    action={<Button onClick={() => tasksQuery.refetch()}>Try again</Button>}
                />
            ) : tasks.length === 0 ? (
                <PageEmpty
                    icon={ClipboardCheck}
                    title={view === "active" ? "No active tasks" : "No completed tasks"}
                    description={view === "active" ? "Add a task above to start tracking work." : "Finished work will appear here."}
                />
            ) : (
                <div className="space-y-3" aria-label={view === "active" ? "Active tasks" : "Completed tasks"}>
                    {tasks.map(task => (
                        <Card key={task.id}>
                            <CardPanel className="flex items-start justify-between gap-4 p-4">
                                <div className="min-w-0">
                                    <Link to="/plans/$planId" params={{ planId: task.id }} className="font-medium hover:underline">
                                        {task.title}
                                    </Link>
                                    {task.description && (
                                        <p className="text-muted-foreground mt-1 text-sm whitespace-pre-wrap">{task.description}</p>
                                    )}
                                    <Badge variant="secondary" className="mt-2">
                                        {PRIORITY_LABEL[(task.metadata.priority as Priority) ?? "normal"] ?? "Normal"}
                                    </Badge>
                                </div>
                                {view === "active" ? (
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        disabled={completeTask.isPending}
                                        onClick={() => completeTask.mutate(task.id)}
                                        aria-label={`Complete ${task.title}`}
                                    >
                                        <Check className="size-4" /> Complete
                                    </Button>
                                ) : task.completedAt ? (
                                    <span className="text-muted-foreground text-xs">
                                        Completed {new Date(task.completedAt).toLocaleDateString()}
                                    </span>
                                ) : null}
                            </CardPanel>
                        </Card>
                    ))}
                </div>
            )}
        </PageContainer>
    );
}
