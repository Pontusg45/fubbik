import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";

type BaseProps = {
    /** Canonical value from the server. The component keeps its own draft state. */
    value: string;
    /** Called when the user commits a distinct value (blur or Enter). */
    onSave: (next: string) => void;
    /** Override the visible placeholder for both read and edit modes. */
    placeholder?: string;
    /** Read-mode wrapper classes (the clickable affordance). */
    className?: string;
    /** Edit-mode input/textarea classes. */
    inputClassName?: string;
    /** Start in edit mode (used when a parent button wants "rename" behaviour). */
    startEditing?: boolean;
    /** Fires when edit mode closes for any reason (save, escape, blur). */
    onEditingChange?: (editing: boolean) => void;
    /** Display-render for the read view. Defaults to rendering `value` as plain text. */
    renderDisplay?: (value: string) => React.ReactNode;
    /** Disable edit mode entirely — useful while a mutation is in-flight. */
    disabled?: boolean;
};

export type InlineEditProps =
    | (BaseProps & { as?: "input"; rows?: never })
    | (BaseProps & { as: "textarea"; rows?: number });

/**
 * Click-to-edit text field. Enter commits, Escape cancels, blur commits.
 * Shift+Enter in textarea mode inserts a newline.
 *
 * Why this exists: roughly ten places in the app re-implement "click to edit,
 * save on blur, revert on Escape" by hand (plan header, task title, plan
 * description, chunk comments, inline tag editor, …). Consolidating here keeps
 * behaviour consistent (cursor-text affordance, focus management, key handling)
 * and halves the touchable surface area for future bugs.
 */
export function InlineEdit(props: InlineEditProps) {
    const {
        value,
        onSave,
        placeholder,
        className,
        inputClassName,
        startEditing = false,
        onEditingChange,
        renderDisplay,
        disabled,
    } = props;
    const as = props.as ?? "input";
    const rows = props.as === "textarea" ? props.rows ?? 4 : undefined;

    const [editing, setEditingState] = useState(startEditing);
    const [draft, setDraft] = useState(value);
    const prevValueRef = useRef(value);

    // Keep the draft in sync when the parent's value changes from the outside
    // (e.g. a refetch after a save). We only reset the draft if we're not
    // currently editing — otherwise we'd stomp the user's in-progress typing.
    useEffect(() => {
        if (!editing && value !== prevValueRef.current) {
            setDraft(value);
            prevValueRef.current = value;
        }
    }, [value, editing]);

    const setEditing = (next: boolean) => {
        setEditingState(next);
        onEditingChange?.(next);
    };

    const commit = () => {
        const trimmed = draft;
        if (trimmed !== value) onSave(trimmed);
        setEditing(false);
    };

    const cancel = () => {
        setDraft(value);
        setEditing(false);
    };

    if (!editing) {
        return (
            <button
                type="button"
                disabled={disabled}
                onClick={() => !disabled && setEditing(true)}
                className={cn(
                    "cursor-text rounded-md border border-transparent p-2 text-left hover:border-border disabled:cursor-default disabled:opacity-60",
                    className,
                )}
                title="Click to edit"
            >
                {value
                    ? renderDisplay?.(value) ?? <span>{value}</span>
                    : <span className="text-muted-foreground">{placeholder ?? "Click to edit"}</span>}
            </button>
        );
    }

    const commonProps = {
        autoFocus: true,
        value: draft,
        onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => setDraft(e.target.value),
        onBlur: commit,
        onKeyDown: (e: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
            if (e.key === "Escape") {
                e.preventDefault();
                cancel();
                return;
            }
            if (e.key === "Enter") {
                // In a textarea, plain Enter inserts a newline; require Cmd/Ctrl
                // to commit. In an input, Enter commits directly.
                if (as === "textarea") {
                    if (e.metaKey || e.ctrlKey) {
                        e.preventDefault();
                        (e.currentTarget as HTMLTextAreaElement).blur();
                    }
                } else {
                    e.preventDefault();
                    (e.currentTarget as HTMLInputElement).blur();
                }
            }
        },
        placeholder,
        className: cn(
            "w-full rounded-md border bg-background p-2 text-sm outline-none focus:ring-1 focus:ring-ring",
            inputClassName,
        ),
    };

    if (as === "textarea") {
        return <textarea {...commonProps} rows={rows} />;
    }
    return <input type="text" {...commonProps} />;
}
