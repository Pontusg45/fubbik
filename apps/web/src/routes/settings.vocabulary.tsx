import { createFileRoute } from "@tanstack/react-router";
import { Library } from "lucide-react";

import { PageContainer, PageHeader } from "@/components/ui/page";
import { Separator } from "@/components/ui/separator";
import { ChunkTypesPanel } from "@/features/vocabularies/chunk-types-panel";
import { ConnectionRelationsPanel } from "@/features/vocabularies/connection-relations-panel";
import { getUser } from "@/functions/get-user";

export const Route = createFileRoute("/settings/vocabulary")({
    component: VocabularyPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest
        }
        return { session };
    }
});

function VocabularyPage() {
    return (
        <PageContainer maxWidth="5xl">
            <PageHeader
                icon={Library}
                title="Vocabulary"
                description="Chunk types and connection relations the graph and editor recognise. Built-in rows are locked; add your own below."
            />
            <div className="space-y-8">
                <ChunkTypesPanel />
                <Separator />
                <ConnectionRelationsPanel />
            </div>
        </PageContainer>
    );
}
