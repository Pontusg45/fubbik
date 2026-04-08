import { useCallback, useState } from "react";
import type { QueryClause, SearchQuery } from "./query-types";

export function useQueryBuilder(initialClauses: QueryClause[] = []) {
    const [clauses, setClauses] = useState<QueryClause[]>(initialClauses);
    const [join, setJoin] = useState<"and" | "or">("and");
    const [sort, setSort] = useState<SearchQuery["sort"]>("relevance");

    const addClause = useCallback((clause: QueryClause) => {
        setClauses(prev => [...prev, clause]);
    }, []);

    const removeClause = useCallback((index: number) => {
        setClauses(prev => prev.filter((_, i) => i !== index));
    }, []);

    const updateClause = useCallback((index: number, clause: QueryClause) => {
        setClauses(prev => prev.map((c, i) => (i === index ? clause : c)));
    }, []);

    const clearAll = useCallback(() => setClauses([]), []);

    const loadClauses = useCallback((newClauses: QueryClause[]) => setClauses(newClauses), []);

    const query: SearchQuery = { clauses, join, sort };
    const hasGraphClauses = clauses.some(c => ["near", "path", "affected-by"].includes(c.field));

    return {
        clauses,
        join,
        sort,
        query,
        hasGraphClauses,
        addClause,
        removeClause,
        updateClause,
        clearAll,
        loadClauses,
        setJoin,
        setSort,
    };
}
