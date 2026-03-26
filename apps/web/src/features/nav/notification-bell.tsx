import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Bell, Check, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function NotificationBell() {
    const queryClient = useQueryClient();
    const navigate = useNavigate();

    const countQuery = useQuery({
        queryKey: ["notifications", "count"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.notifications.count.get());
            } catch {
                return { count: 0 };
            }
        },
        refetchInterval: 30000
    });

    const listQuery = useQuery({
        queryKey: ["notifications", "list"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.notifications.get({ query: { limit: "10" } }));
            } catch {
                return [];
            }
        }
    });

    const markReadMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.notifications({ id }).read.patch());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["notifications"] });
        }
    });

    const markAllReadMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.notifications["read-all"].post());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["notifications"] });
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.notifications({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["notifications"] });
        }
    });

    const unreadCount = countQuery.data?.count ?? 0;
    const notifications = listQuery.data ?? [];

    function handleNotificationClick(n: { id: string; linkTo: string | null; read: boolean }) {
        if (!n.read) {
            markReadMutation.mutate(n.id);
        }
        if (n.linkTo) {
            void navigate({ to: n.linkTo });
        }
    }

    function timeAgo(date: string | Date) {
        const now = Date.now();
        const then = new Date(date).getTime();
        const seconds = Math.floor((now - then) / 1000);
        if (seconds < 60) return "just now";
        const minutes = Math.floor(seconds / 60);
        if (minutes < 60) return `${minutes}m ago`;
        const hours = Math.floor(minutes / 60);
        if (hours < 24) return `${hours}h ago`;
        const days = Math.floor(hours / 24);
        return `${days}d ago`;
    }

    return (
        <Popover>
            <PopoverTrigger>
                <Button variant="ghost" size="icon" className="relative">
                    <Bell className="size-4" />
                    {unreadCount > 0 && (
                        <Badge
                            variant="destructive"
                            size="sm"
                            className="absolute -top-1 -right-1 flex size-4 items-center justify-center rounded-full p-0 text-[10px]"
                        >
                            {unreadCount > 9 ? "9+" : unreadCount}
                        </Badge>
                    )}
                </Button>
            </PopoverTrigger>
            <PopoverContent side="bottom" align="end" className="w-80">
                <div className="mb-2 flex items-center justify-between">
                    <h4 className="text-sm font-semibold">Notifications</h4>
                    {unreadCount > 0 && (
                        <Button
                            variant="ghost"
                            size="xs"
                            onClick={() => markAllReadMutation.mutate()}
                            disabled={markAllReadMutation.isPending}
                        >
                            <Check className="size-3" />
                            Mark all read
                        </Button>
                    )}
                </div>
                <Separator className="mb-2" />
                {notifications.length === 0 ? (
                    <p className="text-muted-foreground py-4 text-center text-sm">No notifications</p>
                ) : (
                    <div className="-mx-4 max-h-80 space-y-0.5 overflow-y-auto px-4">
                        {notifications.map(n => (
                            <button
                                key={n.id}
                                type="button"
                                onClick={() => handleNotificationClick(n)}
                                className={`hover:bg-muted w-full rounded-md p-2 text-left transition-colors ${!n.read ? "bg-muted/50" : ""}`}
                            >
                                <div className="flex items-start justify-between gap-2">
                                    <div className="min-w-0 flex-1">
                                        <p className={`truncate text-sm ${!n.read ? "font-medium" : ""}`}>{n.title}</p>
                                        <p className="text-muted-foreground mt-0.5 line-clamp-2 text-xs">{n.message}</p>
                                        <p className="text-muted-foreground mt-1 text-[10px]">{timeAgo(n.createdAt)}</p>
                                    </div>
                                    <Button
                                        variant="ghost"
                                        size="icon-xs"
                                        onClick={e => {
                                            e.stopPropagation();
                                            deleteMutation.mutate(n.id);
                                        }}
                                        className="shrink-0 text-muted-foreground/50 hover:text-destructive transition-colors"
                                    >
                                        <Trash2 className="size-3" />
                                    </Button>
                                </div>
                            </button>
                        ))}
                    </div>
                )}
            </PopoverContent>
        </Popover>
    );
}
