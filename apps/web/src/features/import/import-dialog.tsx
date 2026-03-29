import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderUp, Upload } from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogClose,
    DialogFooter,
    DialogHeader,
    DialogPopup,
    DialogTitle,
    DialogTrigger
} from "@/components/ui/dialog";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface FileEntry {
    path: string;
    content: string;
}

function readFilesFromInput(fileList: FileList): Promise<FileEntry[]> {
    const mdFiles = Array.from(fileList).filter(f => f.name.endsWith(".md"));
    return Promise.all(
        mdFiles.map(
            file =>
                new Promise<FileEntry>((resolve, reject) => {
                    const reader = new FileReader();
                    reader.onload = () =>
                        resolve({
                            path: file.webkitRelativePath || file.name,
                            content: reader.result as string
                        });
                    reader.onerror = () => reject(reader.error);
                    reader.readAsText(file);
                })
        )
    );
}

export function ImportDocsDialog() {
    const queryClient = useQueryClient();
    const folderInputRef = useRef<HTMLInputElement>(null);
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [codebaseId, setCodebaseId] = useState<string>("");

    const { data: codebases } = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => unwrapEden(await api.api.codebases.get()),
        staleTime: 60_000
    });

    const importMutation = useMutation({
        mutationFn: async (payload: { files: FileEntry[]; codebaseId: string }) =>
            unwrapEden(
                await api.api.chunks["import-docs"].post({
                    files: payload.files,
                    codebaseId: payload.codebaseId
                })
            ),
        onSuccess: data => {
            const msg = `Created: ${data.created} | Skipped: ${data.skipped} | Errors: ${data.errors.length}`;
            if (data.errors.length > 0) {
                toast.warning(msg);
            } else {
                toast.success(msg);
            }
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            setFiles([]);
            setCodebaseId("");
        },
        onError: () => toast.error("Failed to import docs")
    });

    const handleFolderSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files) return;
        const entries = await readFilesFromInput(e.target.files);
        setFiles(entries);
        e.target.value = "";
    };

    const handleImport = () => {
        if (!codebaseId) {
            toast.error("Please select a codebase");
            return;
        }
        if (files.length === 0) {
            toast.error("No markdown files selected");
            return;
        }
        if (files.length > 500) {
            toast.error("Too many files (max 500). Import in smaller batches.");
            return;
        }
        importMutation.mutate({ files, codebaseId });
    };

    return (
        <Dialog
            onOpenChange={open => {
                if (!open) {
                    setFiles([]);
                    setCodebaseId("");
                }
            }}
        >
            <DialogTrigger render={<Button variant="outline" size="sm" />}>
                <FolderUp className="mr-1 size-3.5" />
                Import Docs
            </DialogTrigger>
            <DialogPopup>
                <DialogHeader>
                    <DialogTitle>Import Markdown Docs</DialogTitle>
                </DialogHeader>

                <div className="space-y-4 px-6 py-4">
                    <div>
                        <label className="text-sm font-medium">Folder</label>
                        <input
                            ref={folderInputRef}
                            type="file"
                            // @ts-expect-error webkitdirectory is non-standard
                            webkitdirectory=""
                            className="hidden"
                            onChange={handleFolderSelect}
                        />
                        <div className="mt-1 flex items-center gap-2">
                            <Button variant="outline" size="sm" onClick={() => folderInputRef.current?.click()}>
                                <FolderUp className="mr-1 size-3.5" />
                                Select Folder
                            </Button>
                            {files.length > 0 && (
                                <span className="text-muted-foreground text-sm">
                                    {files.length} markdown file{files.length !== 1 ? "s" : ""} found
                                </span>
                            )}
                        </div>
                    </div>

                    <div>
                        <label className="text-sm font-medium">Codebase</label>
                        <select
                            className="border-input bg-background mt-1 block w-full rounded-md border px-3 py-2 text-sm"
                            value={codebaseId}
                            onChange={e => setCodebaseId(e.target.value)}
                        >
                            <option value="">Select a codebase...</option>
                            {codebases?.map((c: { id: string; name: string }) => (
                                <option key={c.id} value={c.id}>
                                    {c.name}
                                </option>
                            ))}
                        </select>
                    </div>
                </div>

                <DialogFooter variant="bare">
                    <DialogClose render={<Button variant="outline" size="sm" />}>Cancel</DialogClose>
                    <Button
                        size="sm"
                        onClick={handleImport}
                        disabled={importMutation.isPending || files.length === 0 || !codebaseId}
                    >
                        <Upload className="mr-1 size-3.5" />
                        {importMutation.isPending
                            ? "Importing..."
                            : `Import ${files.length} file${files.length !== 1 ? "s" : ""}`}
                    </Button>
                </DialogFooter>
            </DialogPopup>
        </Dialog>
    );
}
