import { Link } from "@tanstack/react-router";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { Badge } from "@/components/ui/badge";

interface VocabularyPopoverProps {
    word: string;
    category: string;
    expects: string[] | null;
    children: ReactNode;
}

export function VocabularyPopover({ word, category, expects, children }: VocabularyPopoverProps) {
    const triggerRef = useRef<HTMLSpanElement>(null);
    const popoverRef = useRef<HTMLDivElement>(null);
    const [open, setOpen] = useState(false);
    const openTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
    const closeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
    const [position, setPosition] = useState<{ top: number; left: number } | null>(null);

    const show = () => {
        clearTimeout(closeTimer.current);
        openTimer.current = setTimeout(() => {
            if (!triggerRef.current) return;
            const rect = triggerRef.current.getBoundingClientRect();
            const popoverHeight = 100; // estimate
            const spaceAbove = rect.top;
            const placeAbove = spaceAbove > popoverHeight + 8;

            setPosition({
                top: placeAbove ? rect.top + window.scrollY - 8 : rect.bottom + window.scrollY + 8,
                left: rect.left + window.scrollX + rect.width / 2
            });
            setOpen(true);
        }, 200);
    };

    const hide = () => {
        clearTimeout(openTimer.current);
        closeTimer.current = setTimeout(() => setOpen(false), 100);
    };

    const keepOpen = () => {
        clearTimeout(closeTimer.current);
    };

    useEffect(() => {
        return () => {
            clearTimeout(openTimer.current);
            clearTimeout(closeTimer.current);
        };
    }, []);

    return (
        <>
            <span
                ref={triggerRef}
                onMouseEnter={show}
                onMouseLeave={hide}
                className="cursor-help underline decoration-dotted underline-offset-2 decoration-muted-foreground/50"
            >
                {children}
            </span>
            {open && position && createPortal(
                <div
                    ref={popoverRef}
                    onMouseEnter={keepOpen}
                    onMouseLeave={hide}
                    className="fixed z-50 w-64 rounded-lg border border-border bg-popover p-3 shadow-md animate-in fade-in-0 zoom-in-95 duration-100"
                    style={{
                        top: position.top,
                        left: position.left,
                        transform: "translateX(-50%)"
                    }}
                >
                    <div className="flex items-center justify-between gap-2 mb-1">
                        <span className="text-sm font-medium text-foreground">{word}</span>
                        <Badge variant="secondary" size="sm" className="text-[10px] shrink-0">
                            {category}
                        </Badge>
                    </div>
                    {expects && expects.length > 0 && (
                        <p className="text-xs text-muted-foreground mb-2">
                            Expects: {expects.join(", ")}
                        </p>
                    )}
                    <Link
                        to="/vocabulary"
                        className="text-xs text-primary hover:text-primary/80 transition-colors"
                    >
                        View in vocabulary →
                    </Link>
                </div>,
                document.body
            )}
        </>
    );
}
