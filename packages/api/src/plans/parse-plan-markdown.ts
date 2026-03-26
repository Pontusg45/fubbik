export interface ParsedPlan {
    title: string;
    description: string;
    steps: Array<{
        description: string;
        order: number;
        taskGroup?: string;
    }>;
}

export function parsePlanMarkdown(markdown: string): ParsedPlan {
    const lines = markdown.split("\n");

    // Extract title from first H1
    let title = "Imported Plan";
    for (const line of lines) {
        if (line.startsWith("# ")) {
            title = line.replace(/^#\s+/, "").trim();
            break;
        }
    }

    // Extract goal/description
    let description = "";
    for (const line of lines) {
        if (line.startsWith("**Goal:**")) {
            description = line.replace("**Goal:**", "").trim();
            break;
        }
    }

    // Extract tasks and steps
    const steps: ParsedPlan["steps"] = [];
    let currentTask = "";
    let order = 0;

    for (const line of lines) {
        // Task header: ## Task N: Name
        if (line.match(/^##\s+Task\s+\d+/)) {
            currentTask = line.replace(/^##\s+/, "").trim();
        }

        // Step: - [ ] **Step N: Description** or - [ ] **Description**
        const stepMatch = line.match(
            /^-\s+\[[ x]\]\s+\*\*(?:Step\s+\d+:\s+)?(.+?)\*\*/
        );
        if (stepMatch) {
            steps.push({
                description: stepMatch[1]!.trim(),
                order: order++,
                taskGroup: currentTask || undefined,
            });
        }
    }

    return { title, description, steps };
}
