import { Badge } from "@/components/ui/badge";

export function GraphFilters({
    types,
    relations,
    activeTypes,
    activeRelations,
    onToggleType,
    onToggleRelation
}: {
    types: string[];
    relations: string[];
    activeTypes: Set<string>;
    activeRelations: Set<string>;
    onToggleType: (type: string) => void;
    onToggleRelation: (relation: string) => void;
}) {
    return (
        <div className="absolute top-4 left-4 z-10 max-w-[200px] space-y-3 rounded-lg border bg-background/80 p-3 backdrop-blur-sm">
            <div>
                <p className="text-muted-foreground mb-1.5 text-[10px] font-medium uppercase">Types</p>
                <div className="flex flex-wrap gap-1">
                    {types.map(t => (
                        <Badge
                            key={t}
                            variant={activeTypes.has(t) ? "default" : "outline"}
                            size="sm"
                            className="cursor-pointer text-[10px]"
                            onClick={() => onToggleType(t)}
                        >
                            {t}
                        </Badge>
                    ))}
                </div>
            </div>
            {relations.length > 0 && (
                <div>
                    <p className="text-muted-foreground mb-1.5 text-[10px] font-medium uppercase">Relations</p>
                    <div className="flex flex-wrap gap-1">
                        {relations.map(r => (
                            <Badge
                                key={r}
                                variant={activeRelations.has(r) ? "default" : "outline"}
                                size="sm"
                                className="cursor-pointer text-[10px]"
                                onClick={() => onToggleRelation(r)}
                            >
                                {r}
                            </Badge>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
