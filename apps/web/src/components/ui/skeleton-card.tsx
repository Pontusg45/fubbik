import { Skeleton } from "@/components/ui/skeleton";

export function SkeletonCard() {
    return (
        <div className="rounded-lg border p-4 space-y-2">
            <Skeleton className="h-3 w-20" />
            <Skeleton className="h-8 w-16" />
        </div>
    );
}
