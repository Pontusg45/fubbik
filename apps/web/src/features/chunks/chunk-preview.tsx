import { Badge } from "@/components/ui/badge";
import { Calendar } from "lucide-react";

interface ChunkPreviewData {
    title: string;
    type: string;
    content: string;
    tags?: Array<{ id: string; name: string }>;
    createdAt: string;
}

export function ChunkPreviewCard({ data }: { data: ChunkPreviewData }) {
    const truncatedContent = data.content
        .split("\n")
        .slice(0, 3)
        .join("\n")
        .slice(0, 200);

    return (
        <div className="bg-popover text-popover-foreground absolute bottom-full left-0 z-50 mb-2 w-72 rounded-lg border p-3 shadow-lg">
            <div className="mb-1.5 flex items-center gap-1.5">
                <Badge variant="secondary" className="font-mono text-[10px]">
                    {data.type}
                </Badge>
                <span className="text-muted-foreground flex items-center gap-1 text-[10px]">
                    <Calendar className="size-2.5" />
                    {new Date(data.createdAt).toLocaleDateString()}
                </span>
            </div>
            <p className="mb-1.5 text-sm font-medium leading-tight">{data.title}</p>
            <p className="text-muted-foreground line-clamp-3 text-xs leading-relaxed">
                {truncatedContent}
                {data.content.length > 200 && "..."}
            </p>
            {data.tags && data.tags.length > 0 && (
                <div className="mt-1.5 flex flex-wrap gap-1">
                    {data.tags.slice(0, 4).map(tag => (
                        <Badge key={tag.id} variant="outline" size="sm" className="text-[9px]">
                            {tag.name}
                        </Badge>
                    ))}
                    {data.tags.length > 4 && (
                        <span className="text-muted-foreground text-[9px]">+{data.tags.length - 4}</span>
                    )}
                </div>
            )}
        </div>
    );
}
