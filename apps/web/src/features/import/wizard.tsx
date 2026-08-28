import { Check } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApiListQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";

import { StepImport } from "./steps/import-step";
import { StepPreview } from "./steps/preview";
import { StepReview } from "./steps/review";
import { StepSelectFiles } from "./steps/select-files";
import type { FileConfig, FileEntry, ImportFileStatus, PreviewFileResult, WizardStep } from "./types";

// ---------------------------------------------------------------------------
// Step indicator
// ---------------------------------------------------------------------------

const STEPS: { label: string }[] = [{ label: "Select Files" }, { label: "Preview & Configure" }, { label: "Review" }, { label: "Import" }];

interface StepIndicatorProps {
    current: WizardStep;
}

function StepIndicator({ current }: StepIndicatorProps) {
    return (
        <div className="mb-8 flex items-center gap-0">
            {STEPS.map((s, i) => {
                const stepNum = (i + 1) as WizardStep;
                const isActive = stepNum === current;
                const isCompleted = stepNum < current;
                const isFuture = stepNum > current;

                return (
                    <div key={stepNum} className="flex items-center">
                        {/* Connector line before step (not before first) */}
                        {i > 0 && <div className="bg-border h-px w-8 shrink-0" />}

                        <div className="flex flex-col items-center gap-1">
                            <div
                                className={`flex size-8 items-center justify-center rounded-full text-sm font-medium ${
                                    isActive
                                        ? "bg-primary text-primary-foreground"
                                        : isCompleted
                                          ? "bg-primary text-primary-foreground"
                                          : "border-border text-muted-foreground border"
                                }`}
                            >
                                {isCompleted ? <Check className="size-4" /> : <span>{stepNum}</span>}
                            </div>
                            <span className={`text-xs whitespace-nowrap ${isFuture ? "text-muted-foreground" : "text-foreground"}`}>
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
    const [spaceId, setSpaceId] = useState<string>("");
    // preview, overrides, importStatus, existingHashes are consumed by steps 2-4
    const [preview, setPreview] = useState<PreviewFileResult[]>([]);
    const [overrides, setOverrides] = useState<Map<string, FileConfig>>(new Map());
    const [importStatus, setImportStatus] = useState<Map<string, ImportFileStatus>>(new Map());
    const [existingHashes, setExistingHashes] = useState<Record<string, string>>({});
    const [previewActivePath, setPreviewActivePath] = useState<string>("");

    const { data: spaces } = useApiListQuery<{ id: string; name: string }>({
        queryKey: ["spaces"],
        queryFn: () => api.api.spaces.get()
    });

    const spaceName = spaces?.find(c => c.id === spaceId)?.name ?? spaceId;

    const canNext = step === 1 ? selectedPaths.size > 0 && spaceId !== "" : step === 2 ? selectedPaths.size > 0 : true;

    const handleNext = () => {
        if (step === 2) setPreviewActivePath("");
        if (step < 4) setStep((step + 1) as WizardStep);
    };

    const handleBack = () => {
        if (step > 1) setStep((step - 1) as WizardStep);
    };

    const handleReset = () => {
        setStep(1);
        setFiles([]);
        setSelectedPaths(new Set());
        setSpaceId("");
        setPreview([]);
        setOverrides(new Map());
        setImportStatus(new Map());
        setExistingHashes({});
        setPreviewActivePath("");
    };

    const nextLabel = step === 1 ? "Preview →" : step === 2 ? "Review →" : "Start Import";

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
                        spaceId={spaceId}
                        onSpaceChange={setSpaceId}
                    />
                )}
                {step === 2 && (
                    <StepPreview
                        files={files}
                        selectedPaths={selectedPaths}
                        spaceId={spaceId}
                        preview={preview}
                        onPreviewLoaded={(results, hashes) => {
                            setPreview(results);
                            setExistingHashes(hashes);
                        }}
                        overrides={overrides}
                        onOverridesChange={setOverrides}
                        initialActivePath={previewActivePath}
                    />
                )}
                {step === 3 && (
                    <StepReview
                        files={files}
                        selectedPaths={selectedPaths}
                        preview={preview}
                        overrides={overrides}
                        existingHashes={existingHashes}
                        spaceName={spaceName}
                        onGoToFile={path => {
                            setPreviewActivePath(path);
                            setStep(2);
                        }}
                    />
                )}
                {step === 4 && (
                    <StepImport
                        files={files}
                        selectedPaths={selectedPaths}
                        spaceId={spaceId}
                        overrides={overrides}
                        importStatus={importStatus}
                        onStatusChange={setImportStatus}
                        onReset={handleReset}
                    />
                )}
            </div>

            {/* Navigation footer — hidden on step 4 */}
            {step !== 4 && (
                <div className="mt-6 flex items-center justify-between border-t pt-4">
                    <Button variant="outline" size="sm" onClick={handleBack} disabled={step === 1}>
                        Back
                    </Button>

                    <span className="text-muted-foreground text-sm">Step {step} of 4</span>

                    <Button size="sm" onClick={handleNext} disabled={!canNext}>
                        {nextLabel}
                    </Button>
                </div>
            )}
        </div>
    );
}
