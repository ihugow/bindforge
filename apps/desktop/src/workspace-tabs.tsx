import { useState, type KeyboardEvent, type ReactNode } from "react";

type Tab = "home" | "profiles" | "devices";
const baseTabs: { id: Tab; label: string }[] = [{ id: "profiles", label: "Perfis" }, { id: "devices", label: "Dispositivos" }];
export function WorkspaceTabs({ home, profiles, devices }: { home?: (visible: boolean) => ReactNode; profiles: (visible: boolean) => ReactNode; devices: ReactNode }) {
  const tabs = home ? [{ id: "home" as Tab, label: "Início" }, ...baseTabs] : baseTabs;
  const [active, setActive] = useState<Tab>(home ? "home" : "profiles");
  function navigate(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1
      : event.key === "ArrowRight" ? (index + 1) % tabs.length : event.key === "ArrowLeft" ? (index + tabs.length - 1) % tabs.length : null;
    if (next === null) return;
    event.preventDefault();
    setActive(tabs[next].id);
    event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  }
  return <>
    <div className="workspace-tabs" role="tablist" aria-label="Seções do aplicativo">
      {tabs.map((tab, index) => <button key={tab.id} type="button" role="tab" id={`tab-${tab.id}`}
        aria-controls={`panel-${tab.id}`} aria-selected={active === tab.id} tabIndex={active === tab.id ? 0 : -1}
        onClick={() => setActive(tab.id)} onKeyDown={event => navigate(event, index)}>{tab.label}</button>)}
    </div>
    {home && <div role="tabpanel" id="panel-home" aria-labelledby="tab-home" hidden={active !== "home"}>{home(active === "home")}</div>}
    <div role="tabpanel" id="panel-profiles" aria-labelledby="tab-profiles" hidden={active !== "profiles"}>
      {profiles(active === "profiles")}
    </div>
    <div role="tabpanel" id="panel-devices" aria-labelledby="tab-devices" hidden={active !== "devices"}>
      {active === "devices" && devices}
    </div>
  </>;
}
