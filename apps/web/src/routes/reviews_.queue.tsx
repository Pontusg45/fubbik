import { createFileRoute } from "@tanstack/react-router";

import { PageContainer, PageHeader } from "@/components/ui/page";
import { ReviewQueueContent } from "@/features/reviews/review-queue-content";
import { getUser } from "@/functions/get-user";

export const Route = createFileRoute("/reviews_/queue")({
    component: ReviewQueuePage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

function ReviewQueuePage() {
    return (
        <PageContainer>
            <PageHeader
                title="Review Queue"
                description="AI-generated chunks awaiting review. Approve, edit, or reject each draft."
            />
            <ReviewQueueContent />
        </PageContainer>
    );
}
