import { createFileRoute } from "@tanstack/react-router";

import { LearningPathForm } from "@/features/learning-paths/learning-path-form";

export const Route = createFileRoute("/learn/new")({ component: NewLearningPath });

function NewLearningPath() {
    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            <h1 className="mb-6 text-2xl font-bold">New Learning Path</h1>
            <LearningPathForm />
        </div>
    );
}
