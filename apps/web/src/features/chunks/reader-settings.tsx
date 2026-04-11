import { Type } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useReaderSettings } from "@/hooks/use-reader-settings";

export function ReaderSettingsPopover() {
    const { settings, update } = useReaderSettings();

    return (
        <Popover>
            <PopoverTrigger render={
                <Button variant="ghost" size="sm" className="gap-1.5">
                    <Type className="size-3.5" />
                    Reader
                </Button>
            } />
            <PopoverContent className="w-64 p-4" align="end">
                <div className="space-y-4">
                    <div>
                        <label className="text-xs font-semibold text-muted-foreground">Font size</label>
                        <div className="mt-1 grid grid-cols-4 gap-1">
                            {(["sm", "base", "lg", "xl"] as const).map(size => (
                                <button
                                    key={size}
                                    type="button"
                                    onClick={() => update({ fontSize: size })}
                                    className={`rounded border px-2 py-1 text-xs transition-colors ${settings.fontSize === size ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                                >
                                    {size.toUpperCase()}
                                </button>
                            ))}
                        </div>
                    </div>
                    <div>
                        <label className="text-xs font-semibold text-muted-foreground">Line height</label>
                        <div className="mt-1 grid grid-cols-3 gap-1">
                            {(["tight", "normal", "relaxed"] as const).map(lh => (
                                <button
                                    key={lh}
                                    type="button"
                                    onClick={() => update({ lineHeight: lh })}
                                    className={`rounded border px-2 py-1 text-xs transition-colors ${settings.lineHeight === lh ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                                >
                                    {lh}
                                </button>
                            ))}
                        </div>
                    </div>
                    <div>
                        <label className="text-xs font-semibold text-muted-foreground">Column width</label>
                        <div className="mt-1 grid grid-cols-3 gap-1">
                            {(["narrow", "normal", "wide"] as const).map(mw => (
                                <button
                                    key={mw}
                                    type="button"
                                    onClick={() => update({ maxWidth: mw })}
                                    className={`rounded border px-2 py-1 text-xs transition-colors ${settings.maxWidth === mw ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                                >
                                    {mw}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>
            </PopoverContent>
        </Popover>
    );
}
