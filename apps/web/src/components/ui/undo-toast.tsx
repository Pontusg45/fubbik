import { toast } from "sonner";

export function undoableAction({
    action,
    undoAction,
    message,
    duration = 5000
}: {
    action: () => Promise<void>;
    undoAction: () => Promise<void>;
    message: string;
    duration?: number;
}) {
    action();
    toast(message, {
        duration,
        action: {
            label: "Undo",
            onClick: () => undoAction()
        }
    });
}
