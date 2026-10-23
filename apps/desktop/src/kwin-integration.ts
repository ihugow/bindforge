export type IntegrationStatus = {
  kwin_available: boolean; daemon_connected: boolean; loaded: boolean;
  focus_received: boolean; legacy_disabled: boolean; message: string;
};
export function integrationSummary(s: IntegrationStatus): string {
  if (!s.kwin_available) return "Aguardando KDE / KWin";
  if (!s.daemon_connected) return "Inicie o daemon para conectar automaticamente";
  if (!s.loaded) return "Ponte temporária desconectada · tentando conectar";
  return s.focus_received ? "Integração automática ativa · foco recebido" : "Ponte temporária conectada · aguardando foco";
}
export function integrationAction(): string { return "Reconectar ao KDE"; }
export function integrationReady(s: IntegrationStatus): boolean {
  return s.kwin_available && s.daemon_connected && s.loaded && s.focus_received;
}
