interface RequirementLike {
    id: string;
    title: string;
    description: string | null;
    steps: Array<{ keyword: string; text: string; params?: Record<string, string> }>;
    status: string;
    priority: string | null;
}

interface GeneratedStep {
    description: string;
    order: number;
    parentStepId?: string;
    requirementId: string;
}

/**
 * Generate plan steps from requirements.
 *
 * - **standard**: one "Implement" + one "Verify" step per requirement,
 *   plus BDD sub-steps if the requirement has Given/When/Then steps.
 * - **detailed**: four steps per requirement (Verify, Implement, Test, Document).
 */
export function generateStepsFromRequirements(
    requirements: RequirementLike[],
    template: "standard" | "detailed" = "standard"
): GeneratedStep[] {
    const steps: GeneratedStep[] = [];
    let order = 0;

    for (const req of requirements) {
        if (template === "standard") {
            // Implement step
            steps.push({
                description: `Implement: ${req.title}`,
                order: order++,
                requirementId: req.id
            });

            // BDD sub-steps from requirement steps (given/when/then)
            if (req.steps && req.steps.length > 0) {
                for (const step of req.steps) {
                    const keyword = step.keyword.charAt(0).toUpperCase() + step.keyword.slice(1);
                    steps.push({
                        description: `  ${keyword} ${step.text}`,
                        order: order++,
                        requirementId: req.id
                    });
                }
            }

            // Verify step
            steps.push({
                description: `Verify: ${req.title}`,
                order: order++,
                requirementId: req.id
            });
        } else {
            // detailed: Verify → Implement → Test → Document
            steps.push({
                description: `Verify requirements: ${req.title}`,
                order: order++,
                requirementId: req.id
            });
            steps.push({
                description: `Implement: ${req.title}`,
                order: order++,
                requirementId: req.id
            });
            steps.push({
                description: `Test: ${req.title}`,
                order: order++,
                requirementId: req.id
            });
            steps.push({
                description: `Document: ${req.title}`,
                order: order++,
                requirementId: req.id
            });
        }
    }

    return steps;
}
