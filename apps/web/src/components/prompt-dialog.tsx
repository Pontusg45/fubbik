import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogPopup,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogFooter,
    DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

interface PromptDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSubmit: (value: string) => void;
    title: string;
    description?: string;
    placeholder?: string;
    defaultValue?: string;
    submitLabel?: string;
}

export function PromptDialog({
    open,
    onOpenChange,
    title,
    description,
    placeholder,
    defaultValue = "",
    submitLabel = "Save",
    onSubmit,
}: PromptDialogProps) {
    const [value, setValue] = useState(defaultValue);

    function handleSubmit() {
        if (!value.trim()) return;
        onSubmit(value.trim());
        setValue(defaultValue);
    }

    return (
        <Dialog
            open={open}
            onOpenChange={(nextOpen) => {
                if (!nextOpen) setValue(defaultValue);
                onOpenChange(nextOpen);
            }}
        >
            <DialogPopup showCloseButton={false}>
                <DialogHeader>
                    <DialogTitle>{title}</DialogTitle>
                    {description && <DialogDescription>{description}</DialogDescription>}
                </DialogHeader>
                <form
                    onSubmit={(e) => {
                        e.preventDefault();
                        handleSubmit();
                    }}
                    className="px-6 pb-2"
                >
                    <Input
                        placeholder={placeholder}
                        value={value}
                        onChange={(e) => setValue((e.target as HTMLInputElement).value)}
                        autoFocus
                    />
                </form>
                <DialogFooter variant="bare">
                    <DialogClose
                        render={<Button variant="outline" />}
                    >
                        Cancel
                    </DialogClose>
                    <Button
                        onClick={handleSubmit}
                        disabled={!value.trim()}
                    >
                        {submitLabel}
                    </Button>
                </DialogFooter>
            </DialogPopup>
        </Dialog>
    );
}
