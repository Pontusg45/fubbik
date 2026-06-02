import { useContext } from "react";

import { ActiveSpaceContext, type ActiveSpaceContextValue } from "./active-space-provider";

export function useActiveSpace(): ActiveSpaceContextValue {
    return useContext(ActiveSpaceContext);
}
