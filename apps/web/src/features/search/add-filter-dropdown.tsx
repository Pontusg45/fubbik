import { Network, Plus } from "lucide-react";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuSub,
    DropdownMenuSubContent,
    DropdownMenuSubTrigger,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { FILTER_CATEGORIES, GRAPH_FIELDS } from "./query-types";
import type { QueryClause } from "./query-types";

const SIMPLE_FIELD_VALUES: Record<string, string[]> = {
    type: ["note", "document", "reference", "schema", "checklist"],
    origin: ["human", "ai"],
    review: ["draft", "reviewed", "approved"],
};

interface AddFilterDropdownProps {
    onAddClause: (clause: QueryClause) => void;
}

export function AddFilterDropdown({ onAddClause }: AddFilterDropdownProps) {
    function promptForValue(field: string): string | null {
        const prompts: Record<string, string> = {
            tag: "Enter tag name:",
            text: "Enter search text:",
            near: "Enter chunk ID and hops (e.g. chunk-id:2):",
            path: "Enter source and target chunk IDs (e.g. id1:id2):",
            "affected-by": "Enter requirement ID:",
            connections: "Enter minimum connection count:",
            updated: "Enter days (e.g. 30 for within 30 days):",
        };
        return window.prompt(prompts[field] ?? `Enter value for ${field}:`);
    }

    function handleSimpleFieldValue(field: string, value: string) {
        onAddClause({ field, operator: "eq", value });
    }

    function handlePromptField(field: string) {
        const value = promptForValue(field);
        if (value && value.trim()) {
            onAddClause({ field, operator: "eq", value: value.trim() });
        }
    }

    return (
        <DropdownMenu>
            <DropdownMenuTrigger className="inline-flex items-center gap-1.5 rounded-md border px-3 py-2 text-sm font-medium transition-colors hover:bg-muted">
                <Plus className="size-3.5" />
                Add filter
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-52">
                {FILTER_CATEGORIES.map((category, catIndex) => (
                    <div key={category.label}>
                        {catIndex > 0 && <DropdownMenuSeparator />}
                        <DropdownMenuLabel className="flex items-center gap-1.5">
                            {category.label === "Graph" && <Network className="size-3 opacity-70" />}
                            {category.label}
                        </DropdownMenuLabel>
                        {category.fields.map(fieldDef => {
                            const isSimple = fieldDef.field in SIMPLE_FIELD_VALUES;
                            const isGraph = GRAPH_FIELDS.includes(fieldDef.field);

                            if (isSimple) {
                                return (
                                    <DropdownMenuSub key={fieldDef.field}>
                                        <DropdownMenuSubTrigger>
                                            {isGraph && <Network className="size-3 opacity-60" />}
                                            <span>{fieldDef.label}</span>
                                        </DropdownMenuSubTrigger>
                                        <DropdownMenuSubContent>
                                            {(SIMPLE_FIELD_VALUES[fieldDef.field] ?? []).map(val => (
                                                <DropdownMenuItem
                                                    key={val}
                                                    onClick={() => handleSimpleFieldValue(fieldDef.field, val)}
                                                >
                                                    {val}
                                                </DropdownMenuItem>
                                            ))}
                                        </DropdownMenuSubContent>
                                    </DropdownMenuSub>
                                );
                            }

                            return (
                                <DropdownMenuItem
                                    key={fieldDef.field}
                                    onClick={() => handlePromptField(fieldDef.field)}
                                >
                                    {isGraph && <Network className="size-3 opacity-60" />}
                                    <span>{fieldDef.label}</span>
                                </DropdownMenuItem>
                            );
                        })}
                    </div>
                ))}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
