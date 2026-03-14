import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface FeatureFlags {
    aiEnabled: boolean;
    enrichmentEnabled: boolean;
    semanticSearchEnabled: boolean;
    aiSuggestionsEnabled: boolean;
    vocabularySuggestEnabled: boolean;
}

export function useFeatureFlags() {
    const { data } = useQuery({
        queryKey: ["feature-flags"],
        queryFn: async () => unwrapEden(await api.api.settings.features.get()) as FeatureFlags,
        staleTime: 60000
    });

    return {
        aiEnabled: data?.aiEnabled ?? true,
        enrichmentEnabled: data?.enrichmentEnabled ?? true,
        semanticSearchEnabled: data?.semanticSearchEnabled ?? true,
        aiSuggestionsEnabled: data?.aiSuggestionsEnabled ?? true,
        vocabularySuggestEnabled: data?.vocabularySuggestEnabled ?? true
    };
}
