import { toast } from "sonner";

export function undoableAction({
    action,
    undoAction,
    message,
    duration = 10000
}: {
    action: () => Promise<void>;
    undoAction: () => Promise<void>;
    message: string;
    duration?: number;
}) {
    void action();
    toast(message, {
        duration,
        action: {
            label: "Undo",
            onClick: () => undoAction()
        }
    });
}
