export type Keyword = "given" | "when" | "then" | "and" | "but";

export interface StepRow {
    keyword: Keyword;
    text: string;
}

export interface StepError {
    step: number;
    error: string;
}

export function validateSteps(steps: StepRow[]): StepError[] {
    const errors: StepError[] = [];
    if (steps.length === 0) {
        errors.push({ step: 0, error: "Must have at least one step" });
        return errors;
    }

    const firstKeyword = steps[0]!.keyword;
    if (firstKeyword === "and" || firstKeyword === "but") {
        errors.push({ step: 0, error: "First step cannot be 'and' or 'but'" });
    } else if (firstKeyword !== "given") {
        errors.push({ step: 0, error: "First step must be 'given'" });
    }

    let phase: "given" | "when" | "then" = "given";
    for (let i = 0; i < steps.length; i++) {
        const { keyword } = steps[i]!;
        if (keyword === "and" || keyword === "but") continue;
        if (keyword === "given") {
            if (phase === "when" || phase === "then") {
                errors.push({ step: i, error: "Cannot use 'given' after 'when' or 'then'" });
            }
        } else if (keyword === "when") {
            if (phase === "then") {
                errors.push({ step: i, error: "Cannot use 'when' after 'then'" });
            } else {
                phase = "when";
            }
        } else if (keyword === "then") {
            if (phase === "given") {
                errors.push({ step: i, error: "'then' must come after 'when' phase" });
            } else {
                phase = "then";
            }
        }
    }

    if (!steps.some(s => s.keyword === "when")) {
        errors.push({ step: -1, error: "Must contain at least one 'when' step" });
    }
    if (!steps.some(s => s.keyword === "then")) {
        errors.push({ step: -1, error: "Must contain at least one 'then' step" });
    }

    return errors;
}
