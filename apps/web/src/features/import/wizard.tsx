"use client";

import { Check } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import type {
    FileConfig,
    FileEntry,
    ImportFileStatus,
    PreviewFileResult,
    WizardStep
} from "./types";
import { StepSelectFiles } from "./steps/select-files";
import { StepPreview } from "./steps/preview";
import { StepReview } from "./steps/review";
import { StepImport } from "./steps/import-step";

// ---------------------------------------------------------------------------
// Step indicator
// ---------------------------------------------------------------------------

const STEPS: { label: string }[] = [
    { label: "Select Files" },
    { label: "Preview & Configure" },
    { label: "Review" },
    { label: "Import" }
];

interface StepIndicatorProps {
    current: WizardStep;
}

function StepIndicator({ current }: StepIndicatorProps) {
    return (
        <div className="flex items-center gap-0 mb-8">
            {STEPS.map((s, i) => {
                const stepNum = (i + 1) as WizardStep;
                const isActive = stepNum === current;
                const isCompleted = stepNum < current;
                const isFuture = stepNum > current;

                return (
                    <div key={stepNum} className="flex items-center">
                        {/* Connector line before step (not before first) */}
                        {i > 0 && <div className="h-px w-8 bg-border shrink-0" />}

                        <div className="flex flex-col items-center gap-1">
                            <div
                                className={`flex size-8 items-center justify-center rounded-full text-sm font-medium ${
                                    isActive
                                        ? "bg-primary text-primary-foreground"
                                        : isCompleted
                                          ? "bg-primary text-primary-foreground"
                                          : "border border-border text-muted-foreground"
                                }`}
                            >
                                {isCompleted ? (
                                    <Check className="size-4" />
                                ) : (
                                    <span>{stepNum}</span>
                                )}
                            </div>
                            <span
                                className={`text-xs whitespace-nowrap ${
                                    isFuture ? "text-muted-foreground" : "text-foreground"
                                }`}
                            >
                                {s.label}
                            </span>
                        </div>
                    </div>
                );
            })}
        </div>
    );
}

// ---------------------------------------------------------------------------
// ImportWizard
// ---------------------------------------------------------------------------

export function ImportWizard() {
    const [step, setStep] = useState<WizardStep>(1);
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
    const [codebaseId, setCodebaseId] = useState<string>("");
    // preview, overrides, importStatus, existingHashes are consumed by steps 2-4
    const [preview, setPreview] = useState<PreviewFileResult[]>([]);
    const [overrides, setOverrides] = useState<Map<string, FileConfig>>(new Map());
    const [importStatus, setImportStatus] = useState<Map<string, ImportFileStatus>>(new Map());
    const [existingHashes, setExistingHashes] = useState<Record<string, string>>({});

    const { data: codebases } = useApiQuery<{ id: string; name: string }[]>({
        queryKey: ["codebases"],
        queryFn: () => api.api.codebases.get(),
        fallback: []
    });

    const codebaseName =
        codebases?.find(c => c.id === codebaseId)?.name ?? codebaseId;

    const canNext =
        step === 1
            ? selectedPaths.size > 0 && codebaseId !== ""
            : step === 2
              ? selectedPaths.size > 0
              : true;

    const handleNext = () => {
        if (step < 4) setStep((step + 1) as WizardStep);
    };

    const handleBack = () => {
        if (step > 1) setStep((step - 1) as WizardStep);
    };

    const handleReset = () => {
        setStep(1);
        setFiles([]);
        setSelectedPaths(new Set());
        setCodebaseId("");
        setPreview([]);
        setOverrides(new Map());
        setImportStatus(new Map());
        setExistingHashes({});
    };

    const nextLabel =
        step === 1 ? "Preview →" : step === 2 ? "Review →" : "Start Import";

    return (
        <div className="flex flex-col">
            <StepIndicator current={step} />

            {/* Step content */}
            <div className="flex-1">
                {step === 1 && (
                    <StepSelectFiles
                        files={files}
                        onFilesChange={setFiles}
                        selectedPaths={selectedPaths}
                        onSelectionChange={setSelectedPaths}
                        codebaseId={codebaseId}
                        onCodebaseChange={setCodebaseId}
                    />
                )}
                {step === 2 && (
                    <StepPreview
                        files={files}
                        selectedPaths={selectedPaths}
                        codebaseId={codebaseId}
                        preview={preview}
                        onPreviewLoaded={(results, hashes) => {
                            setPreview(results);
                            setExistingHashes(hashes);
                        }}
                        overrides={overrides}
                        onOverridesChange={setOverrides}
                    />
                )}
                {step === 3 && (
                    <StepReview
                        files={files}
                        selectedPaths={selectedPaths}
                        preview={preview}
                        overrides={overrides}
                        existingHashes={existingHashes}
                        codebaseName={codebaseName}
                        onGoToFile={_path => {
                            setStep(2);
                        }}
                    />
                )}
                {step === 4 && (
                    <StepImport
                        files={files}
                        selectedPaths={selectedPaths}
                        codebaseId={codebaseId}
                        overrides={overrides}
                        importStatus={importStatus}
                        onStatusChange={setImportStatus}
                        onReset={handleReset}
                    />
                )}
            </div>

            {/* Navigation footer — hidden on step 4 */}
            {step !== 4 && (
                <div className="border-t pt-4 mt-6 flex items-center justify-between">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={handleBack}
                        disabled={step === 1}
                    >
                        Back
                    </Button>

                    <span className="text-sm text-muted-foreground">
                        Step {step} of 4
                    </span>

                    <Button size="sm" onClick={handleNext} disabled={!canNext}>
                        {nextLabel}
                    </Button>
                </div>
            )}
        </div>
    );
}
