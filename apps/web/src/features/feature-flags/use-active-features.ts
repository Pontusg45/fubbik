import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function useActiveFeatures() {
    const queryClient = useQueryClient();

    const { data: activeFeatureIds = [] } = useQuery({
        queryKey: ["features", "active"],
        queryFn: async () => unwrapEden(await api.api.features.active.get()),
        staleTime: 60_000
    });

    const toggleMutation = useMutation({
        mutationFn: async (featureIds: string[]) => {
            unwrapEden(await api.api.features.active.put({ featureIds }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features", "active"] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
        }
    });

    const toggleFeature = (featureId: string) => {
        const current = activeFeatureIds as string[];
        const next = current.includes(featureId)
            ? current.filter(id => id !== featureId)
            : [...current, featureId];
        toggleMutation.mutate(next);
    };

    const isActive = (featureId: string) => (activeFeatureIds as string[]).includes(featureId);

    return { activeFeatureIds: activeFeatureIds as string[], toggleFeature, isActive, isUpdating: toggleMutation.isPending };
}
