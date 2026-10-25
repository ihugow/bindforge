import { useCallback, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { AppIcon } from "./app-icon";
import { WorkspaceNavigation, type NavigationGuard } from "./workspace-navigation";

type Tab = "home" | "profiles" | "devices" | "settings";
export function WorkspaceTabs({ home, profiles, devices, settings, status }: {
  home?: (visible: boolean, manageDevices: () => void) => ReactNode; profiles: (visible: boolean) => ReactNode;
  devices: ReactNode; settings?: (visible: boolean) => ReactNode; status?: ReactNode;
}) {
  const tabs: { id: Tab; label: string; icon: "devices" | "profiles" | "registry" | "settings" }[] = [
    ...(home ? [{ id: "home" as const, label: "Dispositivos", icon: "devices" as const }] : []),
    { id: "profiles", label: "Perfis", icon: "profiles" },
    ...(!home ? [{ id: "devices" as const, label: "Dispositivos", icon: "registry" as const }] : []),
    ...(settings ? [{ id: "settings" as const, label: "Configurações", icon: "settings" as const }] : []),
  ];
  const [active, setActive] = useState<Tab>(home ? "home" : "profiles");
  const guard = useRef<NavigationGuard | null>(null);
  const registerGuard = useCallback((next: NavigationGuard) => {
    guard.current = next;
    return () => { if (guard.current === next) guard.current = null; };
  }, []);
  function select(tab: Tab) {
    const proceed = () => setActive(tab);
    if (guard.current) guard.current(proceed); else proceed();
  }
  function navigate(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1
      : ["ArrowRight", "ArrowDown"].includes(event.key) ? (index + 1) % tabs.length : ["ArrowLeft", "ArrowUp"].includes(event.key) ? (index + tabs.length - 1) % tabs.length : null;
    if (next === null) return;
    event.preventDefault();
    select(tabs[next].id);
    event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  }
  return <WorkspaceNavigation.Provider value={registerGuard}><div className="app-shell">
    <header className="app-topbar"><div className="app-brand"><span className="app-mark"><AppIcon name="spark" /></span><h1>BindForge</h1></div>
      <span className="app-topbar-caption">Seus dispositivos. Suas regras.</span><div className="app-connection" role="status">{settings ? <button className="connection-link" type="button" onClick={() => select("settings")} title="Abrir configurações do serviço">{status ?? "Seu espaço de configuração"}</button> : status ?? "Seu espaço de configuração"}</div></header>
    <div className="workspace-layout"><div className="workspace-sidebar"><span className="sidebar-label">ESPAÇO DE TRABALHO</span>
    <div className="workspace-tabs" role="tablist" aria-orientation="vertical" aria-label="Seções do aplicativo">
      {tabs.map((tab, index) => <button key={tab.id} type="button" role="tab" id={`tab-${tab.id}`}
        aria-controls={`panel-${tab.id}`} aria-selected={active === tab.id || active === "devices" && tab.id === "home"} tabIndex={active === tab.id || active === "devices" && tab.id === "home" ? 0 : -1}
        onClick={() => select(tab.id)} onKeyDown={event => navigate(event, index)}><AppIcon name={tab.icon} /><span>{tab.label}</span></button>)}
    </div>
    <div className="sidebar-footer"><span className="sidebar-orbit" aria-hidden="true"><AppIcon name="spark" /></span><span>Um perfil para<br /><strong>cada contexto.</strong></span></div></div>
    <div className="workspace-content">
    {home && <div role="tabpanel" id="panel-home" aria-labelledby="tab-home" hidden={active !== "home" && active !== "devices"}>
      <div hidden={active === "devices"}>{home(active === "home", () => select("devices"))}</div>
      {active === "devices" && <><button type="button" className="editor-back" onClick={() => select("home")}>← Dispositivos</button>{devices}</>}
    </div>}
    <div role="tabpanel" id="panel-profiles" aria-labelledby="tab-profiles" hidden={active !== "profiles"}>
      {profiles(active === "profiles")}
    </div>
    {!home && <div role="tabpanel" id="panel-devices" aria-labelledby="tab-devices" hidden={active !== "devices"}>
      {active === "devices" && devices}
    </div>}
    {settings && <div role="tabpanel" id="panel-settings" aria-labelledby="tab-settings" hidden={active !== "settings"}>{settings(active === "settings")}</div>}
    </div></div>
  </div></WorkspaceNavigation.Provider>;
}
