import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { LearningPathForm, type LearningPath } from "@/features/learning-paths/learning-path-form";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/learn/$pathId_/edit")({ component: EditLearningPath });

function EditLearningPath() {
    const { pathId } = Route.useParams();
    const query = useQuery({
        queryKey: ["learning-path", pathId],
        queryFn: async () => unwrapEden(await api.api["learning-paths"]({ id: pathId }).get()) as LearningPath
    });
    if (query.isLoading) return <p className="container mx-auto px-4 py-8">Loading learning path…</p>;
    if (query.isError || !query.data)
        return (
            <p role="alert" className="container mx-auto px-4 py-8">
                Learning path not found.
            </p>
        );
    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            <h1 className="mb-6 text-2xl font-bold">Edit Learning Path</h1>
            <LearningPathForm key={pathId} path={query.data} />
        </div>
    );
}
