import { BookOpen, ClipboardCheck, Database, FileText, Lightbulb, Wrench, type LucideIcon } from "lucide-react";

const ICON_MAP: Record<string, LucideIcon> = {
    note: Lightbulb,
    document: BookOpen,
    reference: FileText,
    schema: Database,
    checklist: ClipboardCheck,
    guide: Wrench,
};

export function ChunkTypeIcon({ type, className }: { type: string; className?: string }) {
    const Icon = ICON_MAP[type] ?? FileText;
    return <Icon className={className ?? "size-3.5"} />;
}
