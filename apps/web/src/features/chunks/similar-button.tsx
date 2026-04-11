import { useNavigate } from "@tanstack/react-router";
import { Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";

export function SimilarButton({ chunkTitle }: { chunkTitle: string }) {
    const navigate = useNavigate();

    function handleClick() {
        const q = `similar-to:"${chunkTitle}"`;
        void navigate({ to: "/search", search: { q } as any });
    }

    return (
        <Button variant="ghost" size="sm" onClick={handleClick} className="gap-1.5">
            <Sparkles className="size-3.5" />
            Show similar
        </Button>
    );
}
