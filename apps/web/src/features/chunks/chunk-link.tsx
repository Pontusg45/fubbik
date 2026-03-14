import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useRef, useState } from "react";

import { api } from "@/utils/api";

import { ChunkPreviewCard } from "./chunk-preview";

export function ChunkLink({ chunkId, children }: { chunkId: string; children: React.ReactNode }) {
    const [showPreview, setShowPreview] = useState(false);
    const [hoverEnabled, setHoverEnabled] = useState(false);
    const timeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);

    const { data: previewData } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).get();
            if (error) throw new Error("Failed to load chunk");
            return data;
        },
        enabled: hoverEnabled,
        staleTime: 5 * 60 * 1000
    });

    const handleMouseEnter = () => {
        timeoutRef.current = setTimeout(() => {
            setHoverEnabled(true);
            setShowPreview(true);
        }, 300);
    };

    const handleMouseLeave = () => {
        if (timeoutRef.current) {
            clearTimeout(timeoutRef.current);
        }
        setShowPreview(false);
    };

    return (
        <span className="relative inline-block" onMouseEnter={handleMouseEnter} onMouseLeave={handleMouseLeave}>
            <Link to="/chunks/$chunkId" params={{ chunkId }} className="flex-1 font-medium">
                {children}
            </Link>
            {showPreview && previewData?.chunk && (
                <ChunkPreviewCard
                    data={{
                        title: previewData.chunk.title,
                        type: previewData.chunk.type,
                        content: previewData.chunk.content,
                        tags: (previewData as Record<string, unknown>).tags as
                            | Array<{ id: string; name: string }>
                            | undefined,
                        createdAt: String(previewData.chunk.createdAt)
                    }}
                />
            )}
        </span>
    );
}
