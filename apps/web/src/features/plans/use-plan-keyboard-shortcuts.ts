import { useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";

export function usePlanKeyboardShortcuts() {
    const navigate = useNavigate();

    useEffect(() => {
        function onKey(e: KeyboardEvent) {
            const target = e.target as HTMLElement | null;
            if (target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || target?.isContentEditable) return;

            if (e.key === "a" && !e.metaKey && !e.ctrlKey) {
                document.querySelector<HTMLButtonElement>("button[data-plan-add-task]")?.click();
            } else if ((e.key === "d" || e.key === "D") && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                document.querySelector<HTMLButtonElement>('button[title="Duplicate"]')?.click();
            } else if (e.key === "Escape") {
                void navigate({ to: "/plans" });
            }
        }
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [navigate]);
}
