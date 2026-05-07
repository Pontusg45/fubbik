import { useContext } from "react";

import { ActiveCodebaseContext, type ActiveCodebaseContextValue } from "./active-codebase-provider";

export function useActiveCodebase(): ActiveCodebaseContextValue {
    return useContext(ActiveCodebaseContext);
}
