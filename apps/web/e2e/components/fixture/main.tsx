import { useState } from "react";
import { createRoot } from "react-dom/client";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogTrigger, DialogPopup, DialogTitle, DialogDescription, DialogClose } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, Radio } from "@/components/ui/radio-group";
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Table, TableHeader, TableHead, TableBody, TableRow, TableCell } from "@/components/ui/table";

import "./style.css";
import { FieldFixtures } from "./fields";
import { OverlayFixtures } from "./overlays";

const types = [
    { label: "Note", value: "note" },
    { label: "Document", value: "document" }
];
function Fixture() {
    const [title, setTitle] = useState("");
    const [type, setType] = useState("note");
    const [pinned, setPinned] = useState(false);
    const [notify, setNotify] = useState(false);
    const [selected, setSelected] = useState(false);
    const [sort, setSort] = useState<"ascending" | "descending">("ascending");
    const rows = [
        { title: "Architecture", type: "Note" },
        { title: "Release checklist", type: "Document" }
    ];
    const ordered = sort === "ascending" ? rows : [...rows].reverse();
    return (
        <main className="mx-auto grid max-w-xl gap-6 p-8">
            <h1>Fubbik component contracts</h1>
            <form aria-label="Chunk settings" onSubmit={event => event.preventDefault()} className="grid gap-3">
                <Label htmlFor="title">Title</Label>
                <Input id="title" value={title} onChange={event => setTitle(event.target.value)} />
                <Select items={types} value={type} onValueChange={value => setType(value ?? "note")}>
                    <SelectTrigger aria-label="Chunk type">
                        <SelectValue />
                    </SelectTrigger>
                    <SelectPopup>
                        {types.map(item => (
                            <SelectItem key={item.value} value={item.value}>
                                {item.label}
                            </SelectItem>
                        ))}
                    </SelectPopup>
                </Select>
                <label className="flex gap-2">
                    <Checkbox aria-label="Pinned" checked={pinned} onCheckedChange={setPinned} />
                    Pinned
                </label>
                <label className="flex gap-2">
                    <Switch aria-label="Notifications" checked={notify} onCheckedChange={setNotify} />
                    Notifications
                </label>
            </form>
            <output aria-label="Saved settings">{JSON.stringify({ title, type, pinned, notify })}</output>
            <section aria-label="Other form">
                <Label htmlFor="other-title">Title</Label>
                <Input id="other-title" defaultValue="Untouched" />
            </section>
            <Select multiple items={types}>
                <SelectTrigger aria-label="Included types">
                    <SelectValue placeholder="Choose types" />
                </SelectTrigger>
                <SelectPopup>
                    {types.map(item => (
                        <SelectItem key={item.value} value={item.value}>
                            {item.label}
                        </SelectItem>
                    ))}
                </SelectPopup>
            </Select>
            <RadioGroup aria-label="Visibility" defaultValue="private">
                <label>
                    <Radio aria-label="Private" value="private" />
                    Private
                </label>
                <label>
                    <Radio aria-label="Shared" value="shared" />
                    Shared
                </label>
            </RadioGroup>
            <Checkbox aria-label="Partial selection" indeterminate />
            <Dialog>
                <DialogTrigger render={<Button />}>Edit details</DialogTrigger>
                <DialogPopup>
                    <DialogTitle>Chunk details</DialogTitle>
                    <DialogDescription>Update Chunk metadata</DialogDescription>
                    <Label htmlFor="dialog-title">Title</Label>
                    <Input id="dialog-title" />
                    <DialogClose render={<Button />}>Save</DialogClose>
                </DialogPopup>
            </Dialog>
            <Table aria-label="Chunks">
                <TableHeader>
                    <TableRow>
                        <TableHead>Select</TableHead>
                        <TableHead aria-sort={sort}>
                            <Button onClick={() => setSort(sort === "ascending" ? "descending" : "ascending")}>Title</Button>
                        </TableHead>
                        <TableHead>Type</TableHead>
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {ordered.map(row => (
                        <TableRow key={row.title}>
                            <TableCell>
                                <Checkbox
                                    aria-label={`Select ${row.title}`}
                                    checked={row.title === "Architecture" && selected}
                                    onCheckedChange={setSelected}
                                />
                            </TableCell>
                            <TableCell>{row.title}</TableCell>
                            <TableCell>{row.type}</TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
            <OverlayFixtures />
            <FieldFixtures />
        </main>
    );
}
createRoot(document.getElementById("root")!).render(<Fixture />);
