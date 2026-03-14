import { Link } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";

export function BackLink({ to, label }: { to: string; label: string }) {
    return (
        <div className="mb-6">
            <Link to={to as any} search={{} as any} className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-sm transition-colors">
                <ArrowLeft className="size-3.5" />
                {label}
            </Link>
        </div>
    );
}
