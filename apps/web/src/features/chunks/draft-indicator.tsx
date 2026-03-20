import { CheckCircle } from "lucide-react";

export function DraftIndicator({ lastSaved }: { lastSaved: Date | null }) {
    if (!lastSaved) return null;
    return (
        <span className="text-muted-foreground flex items-center gap-1 text-xs">
            <CheckCircle className="size-3" />
            Draft saved
        </span>
    );
}
