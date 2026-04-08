import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, Network, Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { FILTER_CATEGORIES, GRAPH_FIELDS } from "./query-types";
import type { QueryClause } from "./query-types";

// Fields with preset values — show sub-items, no text input
const PRESET_FIELD_VALUES: Record<string, string[]> = {
    type: ["note", "document", "reference", "schema", "checklist"],
    origin: ["human", "ai"],
    review: ["draft", "approved"],
};

// Fields that use autocomplete
const AUTOCOMPLETE_FIELDS = new Set(["tag", "near", "affected-by"]);

// Fields that use preset buttons instead of free-form text
const CONNECTIONS_PRESETS = ["1", "3", "5"];
const UPDATED_PRESETS = ["7", "30", "90"];

type Mode = "menu" | "input";

// For "path" we need two sequential inputs
type PathStep = "from" | "to";

interface PathState {
    fromValue: string;
    fromLabel: string;
    toValue: string;
}

interface AddFilterDropdownProps {
    onAddClause: (clause: QueryClause) => void;
}

export function AddFilterDropdown({ onAddClause }: AddFilterDropdownProps) {
    const [open, setOpen] = useState(false);
    const [mode, setMode] = useState<Mode>("menu");
    const [selectedField, setSelectedField] = useState<string | null>(null);
    const [inputValue, setInputValue] = useState("");
    // For "path" field
    const [pathStep, setPathStep] = useState<PathStep>("from");
    const [pathState, setPathState] = useState<PathState>({ fromValue: "", fromLabel: "", toValue: "" });

    const inputRef = useRef<HTMLInputElement>(null);

    // Reset state when popover closes
    useEffect(() => {
        if (!open) {
            setMode("menu");
            setSelectedField(null);
            setInputValue("");
            setPathStep("from");
            setPathState({ fromValue: "", fromLabel: "", toValue: "" });
        }
    }, [open]);

    // Auto-focus input when entering input mode
    useEffect(() => {
        if (mode === "input" && inputRef.current) {
            setTimeout(() => inputRef.current?.focus(), 50);
        }
    }, [mode, selectedField, pathStep]);

    // Autocomplete query
    const autocompleteEnabled =
        mode === "input" &&
        selectedField !== null &&
        AUTOCOMPLETE_FIELDS.has(selectedField) &&
        inputValue.length >= 1;

    const { data: suggestions } = useQuery({
        queryKey: ["search-autocomplete", selectedField, inputValue],
        queryFn: async () =>
            unwrapEden(
                await api.api.search.autocomplete.get({
                    query: { field: selectedField!, prefix: inputValue },
                }),
            ),
        enabled: autocompleteEnabled,
        staleTime: 10_000,
    });

    // For path field, the active input determines autocomplete
    const pathAutocompleteEnabled =
        mode === "input" &&
        selectedField === "path" &&
        inputValue.length >= 1;

    const { data: pathSuggestions } = useQuery({
        queryKey: ["search-autocomplete", "near", inputValue],
        queryFn: async () =>
            unwrapEden(
                await api.api.search.autocomplete.get({
                    query: { field: "near", prefix: inputValue },
                }),
            ),
        enabled: pathAutocompleteEnabled,
        staleTime: 10_000,
    });

    function goToInput(field: string) {
        setSelectedField(field);
        setInputValue("");
        setMode("input");
        if (field === "path") {
            setPathStep("from");
            setPathState({ fromValue: "", fromLabel: "", toValue: "" });
        }
    }

    function goBack() {
        setMode("menu");
        setSelectedField(null);
        setInputValue("");
        setPathStep("from");
        setPathState({ fromValue: "", fromLabel: "", toValue: "" });
    }

    function commitClause(field: string, value: string) {
        if (value.trim()) {
            onAddClause({ field, operator: "eq", value: value.trim() });
            setOpen(false);
        }
    }

    function handlePresetValue(field: string, value: string) {
        onAddClause({ field, operator: "eq", value });
        setOpen(false);
    }

    function handleInputKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === "Enter") {
            e.preventDefault();
            if (selectedField === "path") {
                handlePathEnter();
            } else if (selectedField) {
                commitClause(selectedField, inputValue);
            }
        }
        if (e.key === "Escape") {
            e.preventDefault();
            goBack();
        }
    }

    function handlePathEnter() {
        if (pathStep === "from") {
            if (!inputValue.trim()) return;
            setPathState(prev => ({ ...prev, fromValue: inputValue.trim(), fromLabel: inputValue.trim() }));
            setPathStep("to");
            setInputValue("");
        } else {
            if (!inputValue.trim() || !pathState.fromValue) return;
            const value = `${pathState.fromValue}:${inputValue.trim()}`;
            onAddClause({ field: "path", operator: "eq", value });
            setOpen(false);
        }
    }

    function handleSuggestionClick(suggestion: string) {
        if (selectedField === "path") {
            if (pathStep === "from") {
                setPathState(prev => ({ ...prev, fromValue: suggestion, fromLabel: suggestion }));
                setPathStep("to");
                setInputValue("");
            } else {
                const value = `${pathState.fromValue}:${suggestion}`;
                onAddClause({ field: "path", operator: "eq", value });
                setOpen(false);
            }
        } else if (selectedField) {
            commitClause(selectedField, suggestion);
        }
    }

    const activeSuggestions =
        selectedField === "path"
            ? (pathSuggestions as string[] | undefined)
            : (suggestions as string[] | undefined);

    const fieldDefs = FILTER_CATEGORIES.flatMap(c => c.fields);
    const selectedFieldDef = fieldDefs.find(f => f.field === selectedField);

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger className="inline-flex items-center gap-1.5 rounded-md border px-3 py-2 text-sm font-medium transition-colors hover:bg-muted">
                <Plus className="size-3.5" />
                Add filter
            </PopoverTrigger>

            <PopoverContent align="start" className="w-56 p-0" sideOffset={4}>
                {mode === "menu" ? (
                    <MenuView
                        onPresetValue={handlePresetValue}
                        onGoToInput={goToInput}
                    />
                ) : (
                    <InputView
                        field={selectedField!}
                        fieldLabel={selectedFieldDef?.label ?? selectedField ?? ""}
                        inputValue={inputValue}
                        inputRef={inputRef}
                        pathStep={pathStep}
                        pathState={pathState}
                        suggestions={activeSuggestions}
                        onInputChange={setInputValue}
                        onKeyDown={handleInputKeyDown}
                        onSuggestionClick={handleSuggestionClick}
                        onPreset={(value) => commitClause(selectedField!, value)}
                        onBack={goBack}
                    />
                )}
            </PopoverContent>
        </Popover>
    );
}

// ── Menu view ────────────────────────────────────────────────────────────────

interface MenuViewProps {
    onPresetValue: (field: string, value: string) => void;
    onGoToInput: (field: string) => void;
}

function MenuView({ onPresetValue, onGoToInput }: MenuViewProps) {
    return (
        <div className="py-1">
            {FILTER_CATEGORIES.map((category, catIndex) => (
                <div key={category.label}>
                    {catIndex > 0 && <div role="separator" className="bg-border -mx-1 my-1 h-px" />}
                    <div className="text-muted-foreground flex items-center gap-1.5 px-2 py-2 text-xs">
                        {category.label === "Graph" && <Network className="size-3 opacity-70" />}
                        {category.label}
                    </div>
                    {category.fields.map(fieldDef => {
                        const isPreset = fieldDef.field in PRESET_FIELD_VALUES;
                        const isGraph = GRAPH_FIELDS.includes(fieldDef.field);

                        if (isPreset) {
                            return (
                                <PresetFieldRow
                                    key={fieldDef.field}
                                    field={fieldDef.field}
                                    label={fieldDef.label}
                                    isGraph={isGraph}
                                    values={PRESET_FIELD_VALUES[fieldDef.field]!}
                                    onSelect={onPresetValue}
                                />
                            );
                        }

                        return (
                            <button
                                key={fieldDef.field}
                                onClick={() => onGoToInput(fieldDef.field)}
                                className="flex w-full cursor-default items-center gap-2 px-2 py-2 text-xs outline-none hover:bg-accent hover:text-accent-foreground"
                            >
                                {isGraph && <Network className="size-3 opacity-60 shrink-0" />}
                                <span>{fieldDef.label}</span>
                                <span className="ml-auto text-muted-foreground opacity-60">›</span>
                            </button>
                        );
                    })}
                </div>
            ))}
        </div>
    );
}

// ── Preset field row (expandable inline) ─────────────────────────────────────

interface PresetFieldRowProps {
    field: string;
    label: string;
    isGraph: boolean;
    values: string[];
    onSelect: (field: string, value: string) => void;
}

function PresetFieldRow({ field, label, isGraph, values, onSelect }: PresetFieldRowProps) {
    const [expanded, setExpanded] = useState(false);

    return (
        <div>
            <button
                onClick={() => setExpanded(e => !e)}
                className="flex w-full cursor-default items-center gap-2 px-2 py-2 text-xs outline-none hover:bg-accent hover:text-accent-foreground"
            >
                {isGraph && <Network className="size-3 opacity-60 shrink-0" />}
                <span>{label}</span>
                <span className="ml-auto text-muted-foreground opacity-60">{expanded ? "▾" : "›"}</span>
            </button>
            {expanded && (
                <div className="pb-1">
                    {values.map(val => (
                        <button
                            key={val}
                            onClick={() => onSelect(field, val)}
                            className="flex w-full cursor-default items-center pl-6 pr-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground"
                        >
                            {val}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

// ── Input view ───────────────────────────────────────────────────────────────

interface InputViewProps {
    field: string;
    fieldLabel: string;
    inputValue: string;
    inputRef: React.RefObject<HTMLInputElement | null>;
    pathStep: PathStep;
    pathState: PathState;
    suggestions: string[] | undefined;
    onInputChange: (v: string) => void;
    onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
    onSuggestionClick: (v: string) => void;
    onPreset: (v: string) => void;
    onBack: () => void;
}

function InputView({
    field,
    fieldLabel,
    inputValue,
    inputRef,
    pathStep,
    pathState,
    suggestions,
    onInputChange,
    onKeyDown,
    onSuggestionClick,
    onPreset,
    onBack,
}: InputViewProps) {
    const placeholder = getPlaceholder(field, pathStep);

    return (
        <div className="flex flex-col gap-0">
            {/* Header */}
            <div className="flex items-center gap-1 border-b px-2 py-2">
                <button
                    onClick={onBack}
                    className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground outline-none"
                    title="Back"
                >
                    <ArrowLeft className="size-3" />
                </button>
                <span className="text-xs font-medium">
                    {field === "path"
                        ? pathStep === "from"
                            ? "From chunk"
                            : "To chunk"
                        : fieldLabel}
                </span>
                {field === "path" && pathStep === "to" && (
                    <span className="ml-auto text-xs text-muted-foreground truncate max-w-24" title={pathState.fromLabel}>
                        from: {pathState.fromLabel}
                    </span>
                )}
            </div>

            {/* Preset buttons for connections / updated */}
            {field === "connections" && (
                <div className="flex gap-1.5 px-2 py-2">
                    {CONNECTIONS_PRESETS.map(n => (
                        <Button key={n} size="sm" variant="outline" className="flex-1 text-xs" onClick={() => onPreset(n)}>
                            {n}+
                        </Button>
                    ))}
                </div>
            )}
            {field === "updated" && (
                <div className="flex gap-1.5 px-2 py-2">
                    {UPDATED_PRESETS.map(d => (
                        <Button key={d} size="sm" variant="outline" className="flex-1 text-xs" onClick={() => onPreset(d)}>
                            {d}d
                        </Button>
                    ))}
                </div>
            )}

            {/* Text input (not shown for connections/updated since they use only presets) */}
            {field !== "connections" && field !== "updated" && (
                <div className="px-2 py-2">
                    <input
                        ref={inputRef}
                        type="text"
                        value={inputValue}
                        onChange={e => onInputChange(e.target.value)}
                        onKeyDown={onKeyDown}
                        placeholder={placeholder}
                        className="w-full rounded border bg-background px-2 py-1.5 text-xs outline-none focus:ring-1 focus:ring-ring"
                    />
                </div>
            )}

            {/* Autocomplete suggestions */}
            {suggestions && suggestions.length > 0 && (
                <div className="max-h-40 overflow-y-auto border-t">
                    {suggestions.map((s, i) => (
                        <button
                            key={i}
                            onMouseDown={e => {
                                // Use mousedown to fire before blur
                                e.preventDefault();
                                onSuggestionClick(s);
                            }}
                            className="flex w-full cursor-default items-center px-2 py-1.5 text-xs outline-none hover:bg-accent hover:text-accent-foreground"
                        >
                            {s}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

function getPlaceholder(field: string, pathStep: PathStep): string {
    if (field === "path") {
        return pathStep === "from" ? "Chunk name or ID…" : "Chunk name or ID…";
    }
    const map: Record<string, string> = {
        tag: "Tag name…",
        text: "Search text…",
        near: "Chunk name or ID…",
        "affected-by": "Requirement name or ID…",
    };
    return map[field] ?? `Value for ${field}…`;
}
