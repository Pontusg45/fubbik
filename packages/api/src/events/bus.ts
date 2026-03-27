type EventHandler<T = unknown> = (payload: T) => void | Promise<void>;

class EventBus {
    private handlers = new Map<string, EventHandler[]>();

    on<T>(event: string, handler: EventHandler<T>) {
        const existing = this.handlers.get(event) ?? [];
        existing.push(handler as EventHandler);
        this.handlers.set(event, existing);
        return () => {
            const list = this.handlers.get(event) ?? [];
            this.handlers.set(
                event,
                list.filter(h => h !== handler)
            );
        };
    }

    async emit<T>(event: string, payload: T) {
        const handlers = this.handlers.get(event) ?? [];
        for (const handler of handlers) {
            try {
                await handler(payload);
            } catch (err) {
                console.error(`[event:${event}] handler error:`, err);
            }
        }
    }
}

export const events = new EventBus();

export const EVENTS = {
    CHUNK_CREATED: "chunk:created",
    CHUNK_UPDATED: "chunk:updated",
    CHUNK_DELETED: "chunk:deleted",
    PLAN_COMPLETED: "plan:completed",
    SESSION_COMPLETED: "session:completed",
    REQUIREMENT_STATUS_CHANGED: "requirement:status-changed",
} as const;
