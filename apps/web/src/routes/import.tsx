import { createFileRoute } from "@tanstack/react-router";
import { Upload } from "lucide-react";
import { useState } from "react";

import { PageContainer, PageHeader } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { ImportQuickMode } from "@/features/import/quick-mode";
import { ImportWizard } from "@/features/import/wizard";

export const Route = createFileRoute("/import")({
    component: ImportPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

type ImportMode = "quick" | "wizard";

const STORAGE_KEY = "import-mode";

function getInitialMode(): ImportMode {
    if (typeof localStorage !== "undefined") {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored === "quick" || stored === "wizard") return stored;
    }
    return "quick";
}

function ImportPage() {
    const [mode, setMode] = useState<ImportMode>(getInitialMode);

    const handleModeChange = (next: ImportMode) => {
        setMode(next);
        if (typeof localStorage !== "undefined") {
            localStorage.setItem(STORAGE_KEY, next);
        }
    };

    return (
        <PageContainer maxWidth="5xl">
            <PageHeader
                icon={Upload}
                title="Import Docs"
                description="Import a folder of markdown files as knowledge chunks"
                actions={
                    <div className="flex rounded-md border overflow-hidden text-sm">
                        <button
                            type="button"
                            className={`px-3 py-1.5 transition-colors ${
                                mode === "quick"
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-background text-muted-foreground hover:text-foreground"
                            }`}
                            onClick={() => handleModeChange("quick")}
                        >
                            Quick
                        </button>
                        <button
                            type="button"
                            className={`px-3 py-1.5 transition-colors border-l ${
                                mode === "wizard"
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-background text-muted-foreground hover:text-foreground"
                            }`}
                            onClick={() => handleModeChange("wizard")}
                        >
                            Wizard
                        </button>
                    </div>
                }
            />

            {mode === "quick" && <ImportQuickMode />}
            {mode === "wizard" && <ImportWizard />}
        </PageContainer>
    );
}
