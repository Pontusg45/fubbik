import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Accordion, AccordionItem, AccordionTrigger, AccordionPanel } from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleTrigger, CollapsiblePanel } from "@/components/ui/collapsible";
import {
    DropdownMenu,
    DropdownMenuTrigger,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuCheckboxItem,
    DropdownMenuRadioGroup,
    DropdownMenuRadioItem
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Popover, PopoverTrigger, PopoverPopup, PopoverTitle } from "@/components/ui/popover";
import { Sheet, SheetTrigger, SheetPopup, SheetTitle, SheetDescription } from "@/components/ui/sheet";

export function OverlayFixtures() {
    const [archived, setArchived] = useState(false);
    const [view, setView] = useState("list");
    const [action, setAction] = useState("None");
    const [confirmOpen, setConfirmOpen] = useState(false);
    const [deleted, setDeleted] = useState(false);
    return (
        <section aria-label="Overlay contracts" className="grid gap-4">
            <Button onClick={() => setConfirmOpen(true)}>Delete fixture chunk</Button>
            <ConfirmDialog
                open={confirmOpen}
                onOpenChange={setConfirmOpen}
                title="Delete fixture chunk?"
                description="Delete this test chunk permanently."
                confirmLabel="Delete"
                onConfirm={() => {
                    setDeleted(true);
                    setConfirmOpen(false);
                }}
            />
            <output aria-label="Deletion result">{deleted ? "Deleted" : "Preserved"}</output>
            <DropdownMenu>
                <DropdownMenuTrigger render={<Button />}>Chunk actions</DropdownMenuTrigger>
                <DropdownMenuContent>
                    <DropdownMenuCheckboxItem checked={archived} onCheckedChange={setArchived} closeOnClick={false}>
                        Show archived
                    </DropdownMenuCheckboxItem>
                    <DropdownMenuRadioGroup value={view} onValueChange={setView}>
                        <DropdownMenuRadioItem value="list" closeOnClick={false}>
                            List view
                        </DropdownMenuRadioItem>
                        <DropdownMenuRadioItem value="grid" closeOnClick={false}>
                            Grid view
                        </DropdownMenuRadioItem>
                    </DropdownMenuRadioGroup>
                    <DropdownMenuItem disabled>Delete chunk</DropdownMenuItem>
                    <DropdownMenuItem onClick={() => setAction("Exported")}>Export chunk</DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
            <output aria-label="Menu settings">{JSON.stringify({ archived, view, action })}</output>
            <Popover>
                <PopoverTrigger render={<Button />}>Edit summary</PopoverTrigger>
                <PopoverPopup>
                    <PopoverTitle>Chunk summary</PopoverTitle>
                    <Label htmlFor="summary-title">Title</Label>
                    <Input id="summary-title" />
                </PopoverPopup>
            </Popover>
            <Sheet>
                <SheetTrigger render={<Button />}>Open inspector</SheetTrigger>
                <SheetPopup>
                    <SheetTitle>Chunk inspector</SheetTitle>
                    <SheetDescription>Inspect chunk metadata</SheetDescription>
                    <Label htmlFor="inspector-title">Title</Label>
                    <Input id="inspector-title" />
                </SheetPopup>
            </Sheet>
            <Collapsible>
                <CollapsibleTrigger render={<Button />}>Decision context</CollapsibleTrigger>
                <CollapsiblePanel>Considered PostgreSQL and SQLite.</CollapsiblePanel>
            </Collapsible>
            <Accordion>
                <AccordionItem value="rationale">
                    <AccordionTrigger>Rationale</AccordionTrigger>
                    <AccordionPanel>Transactions keep chunk writes atomic.</AccordionPanel>
                </AccordionItem>
            </Accordion>
        </section>
    );
}
