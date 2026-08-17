import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link2Off, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useApiQuery } from "@/hooks/use-api-query";
import { legacyApi } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface CellPanelProps {
    matrixId: string;
    cellId: string;
    ruleTitle: string;
    dimensionName: string;
    onClose: () => void;
}

interface CellRequirement {
    id: string;
    title: string;
    status: string;
}

interface CellCodeLink {
    id: string;
    cellId: string;
    kind: "file" | "symbol" | "test";
    ref: string;
    createdAt: string;
}

interface CellTestResult {
    id: string;
    cellId: string;
    testRef: string;
    status: "pass" | "fail";
    detail: string | null;
    runAt: string;
}

const STATUS_BADGE: Record<string, "success" | "destructive" | "secondary"> = {
    passing: "success",
    failing: "destructive",
    untested: "secondary"
};

const CODE_KIND_LABEL: Record<string, string> = {
    file: "File",
    symbol: "Symbol",
    test: "Test"
};

function formatDate(value: string): string {
    const d = new Date(value);
    if (Number.isNaN(d.getTime())) return value;
    return d.toLocaleString();
}

export function CellPanel({ matrixId, cellId, ruleTitle, dimensionName, onClose }: CellPanelProps) {
    const queryClient = useQueryClient();
    const [requirementId, setRequirementId] = useState("");

    // Code link form state
    const [codeKind, setCodeKind] = useState<"file" | "symbol" | "test">("file");
    const [codeRef, setCodeRef] = useState("");

    // Test result form state
    const [testRef, setTestRef] = useState("");
    const [testStatus, setTestStatus] = useState<"pass" | "fail">("pass");
    const [testDetail, setTestDetail] = useState("");

    const requirementsQuery = useApiQuery<CellRequirement[]>({
        queryKey: ["matrix-cell-requirements", cellId],
        queryFn: () => legacyApi.api.matrices({ id: matrixId }).cells({ cellId }).requirements.get(),
        fallback: []
    });

    const codeQuery = useApiQuery<CellCodeLink[]>({
        queryKey: ["matrix-cell-code", cellId],
        queryFn: () => legacyApi.api.matrices({ id: matrixId }).cells({ cellId }).code.get(),
        fallback: []
    });

    const testResultsQuery = useApiQuery<CellTestResult[]>({
        queryKey: ["matrix-cell-test-results", cellId],
        queryFn: () => legacyApi.api.matrices({ id: matrixId }).cells({ cellId })["test-results"].get(),
        fallback: []
    });

    const linkMutation = useMutation({
        mutationFn: async (reqId: string) =>
            unwrapEden(await legacyApi.api.matrices({ id: matrixId }).cells({ cellId }).requirements.post({ requirementId: reqId })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-requirements", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setRequirementId("");
            toast.success("Requirement linked");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to link requirement";
            toast.error(msg);
        }
    });

    const unlinkMutation = useMutation({
        mutationFn: async (reqId: string) =>
            unwrapEden(await legacyApi.api.matrices({ id: matrixId }).cells({ cellId }).requirements({ reqId }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-requirements", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            toast.success("Requirement unlinked");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to unlink requirement";
            toast.error(msg);
        }
    });

    const addCodeMutation = useMutation({
        mutationFn: async (body: { kind: "file" | "symbol" | "test"; ref: string }) =>
            unwrapEden(await legacyApi.api.matrices({ id: matrixId }).cells({ cellId }).code.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-code", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setCodeRef("");
            toast.success("Code link added");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to add code link";
            toast.error(msg);
        }
    });

    const removeCodeMutation = useMutation({
        mutationFn: async (codeId: string) =>
            unwrapEden(await legacyApi.api.matrices({ id: matrixId }).cells({ cellId }).code({ codeId }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-code", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            toast.success("Code link removed");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to remove code link";
            toast.error(msg);
        }
    });

    const addTestResultMutation = useMutation({
        mutationFn: async (body: { testRef: string; status: "pass" | "fail"; detail?: string }) =>
            unwrapEden(await legacyApi.api.matrices({ id: matrixId }).cells({ cellId })["test-results"].post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-test-results", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setTestRef("");
            setTestDetail("");
            setTestStatus("pass");
            toast.success("Test result recorded");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to record test result";
            toast.error(msg);
        }
    });

    function handleLink(e: React.FormEvent) {
        e.preventDefault();
        const id = requirementId.trim();
        if (!id) return;
        linkMutation.mutate(id);
    }

    function handleAddCode(e: React.FormEvent) {
        e.preventDefault();
        const ref = codeRef.trim();
        if (!ref) return;
        addCodeMutation.mutate({ kind: codeKind, ref });
    }

    function handleAddTestResult(e: React.FormEvent) {
        e.preventDefault();
        const ref = testRef.trim();
        if (!ref) return;
        const detail = testDetail.trim();
        addTestResultMutation.mutate({ testRef: ref, status: testStatus, ...(detail ? { detail } : {}) });
    }

    const requirements = Array.isArray(requirementsQuery.data) ? requirementsQuery.data : [];
    const codeLinks = Array.isArray(codeQuery.data) ? codeQuery.data : [];
    const testResults = Array.isArray(testResultsQuery.data) ? testResultsQuery.data : [];

    return (
        <div className="bg-background fixed inset-y-0 right-0 z-40 flex w-full max-w-sm flex-col border-l shadow-lg">
            {/* Header */}
            <div className="flex items-start justify-between gap-3 border-b px-4 py-4">
                <div className="min-w-0">
                    <h3 className="leading-tight font-semibold">{ruleTitle}</h3>
                    <p className="text-muted-foreground mt-0.5 text-sm">{dimensionName}</p>
                </div>
                <Button size="icon-xs" variant="ghost" onClick={onClose} aria-label="Close panel">
                    <X className="size-4" />
                </Button>
            </div>

            <div className="flex-1 overflow-y-auto px-4 py-3">
                {/* Requirements list */}
                <section>
                    <h4 className="text-muted-foreground mb-2 text-xs font-semibold tracking-wide uppercase">
                        Requirements ({requirements.length})
                    </h4>
                    {requirements.length === 0 ? (
                        <p className="text-muted-foreground py-4 text-center text-sm">No requirements linked to this cell.</p>
                    ) : (
                        <ul className="space-y-2">
                            {requirements.map(req => (
                                <li key={req.id} className="flex items-center gap-2 rounded-md border px-3 py-2">
                                    <div className="min-w-0 flex-1">
                                        <span className="line-clamp-1 text-sm font-medium">{req.title}</span>
                                    </div>
                                    <Badge variant={STATUS_BADGE[req.status] ?? "secondary"} size="sm">
                                        {req.status}
                                    </Badge>
                                    <Button
                                        size="icon-xs"
                                        variant="ghost"
                                        onClick={() => unlinkMutation.mutate(req.id)}
                                        disabled={unlinkMutation.isPending}
                                        title="Unlink requirement"
                                        aria-label={`Unlink ${req.title}`}
                                    >
                                        <Link2Off className="size-3.5" />
                                    </Button>
                                </li>
                            ))}
                        </ul>
                    )}
                    {/* Link input */}
                    <form onSubmit={handleLink} className="mt-2 flex gap-2">
                        <Input
                            placeholder="Requirement ID..."
                            value={requirementId}
                            onChange={e => setRequirementId(e.target.value)}
                            size="sm"
                        />
                        <Button type="submit" size="sm" disabled={!requirementId.trim() || linkMutation.isPending}>
                            <Plus className="size-3.5" />
                            Link
                        </Button>
                    </form>
                </section>

                {/* Code & Tests */}
                <section className="mt-6 border-t pt-4">
                    <h4 className="text-muted-foreground mb-2 text-xs font-semibold tracking-wide uppercase">
                        Code &amp; Tests ({codeLinks.length})
                    </h4>
                    {codeLinks.length === 0 ? (
                        <p className="text-muted-foreground py-4 text-center text-sm">No code linked to this cell.</p>
                    ) : (
                        <ul className="space-y-2">
                            {codeLinks.map(link => (
                                <li key={link.id} className="flex items-center gap-2 rounded-md border px-3 py-2">
                                    <Badge variant="outline" size="sm">
                                        {CODE_KIND_LABEL[link.kind] ?? link.kind}
                                    </Badge>
                                    <div className="min-w-0 flex-1">
                                        <span className="line-clamp-1 font-mono text-xs" title={link.ref}>
                                            {link.ref}
                                        </span>
                                    </div>
                                    <Button
                                        size="icon-xs"
                                        variant="ghost"
                                        onClick={() => removeCodeMutation.mutate(link.id)}
                                        disabled={removeCodeMutation.isPending}
                                        title="Remove code link"
                                        aria-label={`Remove ${link.ref}`}
                                    >
                                        <Trash2 className="size-3.5" />
                                    </Button>
                                </li>
                            ))}
                        </ul>
                    )}
                    {/* Add code link form */}
                    <form onSubmit={handleAddCode} className="mt-2 flex gap-2">
                        <Select value={codeKind} onValueChange={val => val && setCodeKind(val as "file" | "symbol" | "test")}>
                            <SelectTrigger size="sm" className="w-28 shrink-0">
                                <SelectValue>{CODE_KIND_LABEL[codeKind]}</SelectValue>
                            </SelectTrigger>
                            <SelectPopup>
                                <SelectItem value="file">File</SelectItem>
                                <SelectItem value="symbol">Symbol</SelectItem>
                                <SelectItem value="test">Test</SelectItem>
                            </SelectPopup>
                        </Select>
                        <Input placeholder="Reference..." value={codeRef} onChange={e => setCodeRef(e.target.value)} size="sm" />
                        <Button type="submit" size="sm" disabled={!codeRef.trim() || addCodeMutation.isPending}>
                            <Plus className="size-3.5" />
                        </Button>
                    </form>
                </section>

                {/* Test results */}
                <section className="mt-6 border-t pt-4">
                    <h4 className="text-muted-foreground mb-2 text-xs font-semibold tracking-wide uppercase">
                        Test results ({testResults.length})
                    </h4>
                    {testResults.length === 0 ? (
                        <p className="text-muted-foreground py-4 text-center text-sm">No test results recorded for this cell.</p>
                    ) : (
                        <ul className="space-y-2">
                            {testResults.map(result => (
                                <li key={result.id} className="rounded-md border px-3 py-2">
                                    <div className="flex items-center gap-2">
                                        <Badge variant={result.status === "pass" ? "success" : "destructive"} size="sm">
                                            {result.status}
                                        </Badge>
                                        <div className="min-w-0 flex-1">
                                            <span className="line-clamp-1 font-mono text-xs" title={result.testRef}>
                                                {result.testRef}
                                            </span>
                                        </div>
                                        <span className="text-muted-foreground shrink-0 text-xs">{formatDate(result.runAt)}</span>
                                    </div>
                                    {result.detail && (
                                        <p className="text-muted-foreground mt-1 text-xs whitespace-pre-wrap">{result.detail}</p>
                                    )}
                                </li>
                            ))}
                        </ul>
                    )}
                    {/* Record test result form */}
                    <form onSubmit={handleAddTestResult} className="mt-2 space-y-2">
                        <div className="flex gap-2">
                            <Input placeholder="Test ref..." value={testRef} onChange={e => setTestRef(e.target.value)} size="sm" />
                            <Select value={testStatus} onValueChange={val => val && setTestStatus(val as "pass" | "fail")}>
                                <SelectTrigger size="sm" className="w-24 shrink-0">
                                    <SelectValue>{testStatus}</SelectValue>
                                </SelectTrigger>
                                <SelectPopup>
                                    <SelectItem value="pass">pass</SelectItem>
                                    <SelectItem value="fail">fail</SelectItem>
                                </SelectPopup>
                            </Select>
                        </div>
                        <div className="flex gap-2">
                            <Input
                                placeholder="Detail (optional)..."
                                value={testDetail}
                                onChange={e => setTestDetail(e.target.value)}
                                size="sm"
                            />
                            <Button type="submit" size="sm" disabled={!testRef.trim() || addTestResultMutation.isPending}>
                                <Plus className="size-3.5" />
                                Record
                            </Button>
                        </div>
                    </form>
                </section>
            </div>
        </div>
    );
}
