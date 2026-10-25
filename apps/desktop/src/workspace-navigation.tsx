import { createContext, useContext } from "react";

export type NavigationGuard = (proceed: () => void) => void;
export const WorkspaceNavigation = createContext<(guard: NavigationGuard) => () => void>(() => () => {});
export const useWorkspaceNavigation = () => useContext(WorkspaceNavigation);
