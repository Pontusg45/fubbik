import { Badge } from "@/components/ui/badge";

interface TagTypeInfo {
    id: string;
    name: string;
    color: string;
    tagCount: number;
}

export function GraphFilters({
    types,
    relations,
    activeTypes,
    activeRelations,
    onToggleType,
    onToggleRelation,
    tagTypes,
    activeTagTypeIds,
    onToggleTagType
}: {
    types: string[];
    relations: string[];
    activeTypes: Set<string>;
    activeRelations: Set<string>;
    onToggleType: (type: string) => void;
    onToggleRelation: (relation: string) => void;
    tagTypes?: TagTypeInfo[];
    activeTagTypeIds?: Set<string>;
    onToggleTagType?: (id: string) => void;
}) {
    return (
        <div className="bg-background/80 absolute top-4 left-4 z-10 max-w-[200px] space-y-3 rounded-lg border p-3 backdrop-blur-sm">
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
            {tagTypes && tagTypes.length > 0 && onToggleTagType && activeTagTypeIds && (
                <div className="border-t pt-3">
                    <p className="text-muted-foreground mb-1.5 text-[10px] font-medium uppercase">Group by Tag Type</p>
                    <div className="flex flex-wrap gap-1">
                        {tagTypes.map(tt => (
                            <Badge
                                key={tt.id}
                                variant={activeTagTypeIds.has(tt.id) ? "default" : "outline"}
                                size="sm"
                                className="cursor-pointer text-[10px]"
                                style={
                                    activeTagTypeIds.has(tt.id)
                                        ? { backgroundColor: tt.color, borderColor: tt.color, color: "#fff" }
                                        : { borderColor: tt.color, color: tt.color }
                                }
                                onClick={() => onToggleTagType(tt.id)}
                            >
                                {tt.name}
                            </Badge>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
