import { AppButton } from "./app-button";
import { useRef, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Modal } from "./modal";
import { profileApi } from "./profile-api";
import type { Config } from "./profiles";

type Preview = { name: string; contents: string; config: Config; revision: number };
export function BackupPanel({ native, onRestored }: { native: boolean; onRestored: (config: Config) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  async function exportBackup() {
    setBusy(true); setError(""); setMessage("");
    try {
      const path = await invoke<string | null>("export_configuration");
      if (path) setMessage(`Backup exportado para ${path}. Guarde uma cópia fora deste computador antes de formatar.`);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function inspect(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]; event.target.value = "";
    if (!file) return;
    setBusy(true); setError(""); setMessage("");
    try {
      if (file.size > 1024 * 1024) throw new Error("Backup excede 1 MiB.");
      const contents = await file.text();
      const config = await invoke<Config>("inspect_backup", { contents });
      const current = await profileApi.load();
      setPreview({ name: file.name, contents, config, revision: current.revision });
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function restore() {
    if (!preview || busy) return;
    setBusy(true); setError("");
    try {
      const config = await invoke<Config>("import_configuration", { contents: preview.contents, expectedRevision: preview.revision });
      onRestored(config); setPreview(null);
      setMessage("Backup importado. Confira as conexões dos dispositivos. O serviço recarrega os perfis automaticamente quando está em execução.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  return <section className="integration-panel" aria-labelledby="backup-heading" aria-busy={busy}>
    <div className="section-heading"><div><span className="eyebrow">SALVAR E RESTAURAR</span><h2 id="backup-heading">Backup das configurações</h2></div></div>
    <p>Exporte dispositivos, conexões, perfis, ícones, associações de aplicativos e todas as ações para um arquivo JSON.</p>
    <p className="hint">Após reinstalar, importe o backup e confira as conexões. Permissões de acesso e inicialização do serviço no KDE devem ser configuradas neste computador.</p>
    <div className="button-row">
      <AppButton type="button" disabled={!native || busy || !!preview} onClick={() => void exportBackup()}>Exportar configurações</AppButton>
      <AppButton type="button" className="secondary-button" disabled={!native || busy || !!preview} onClick={() => input.current?.click()}>Importar configurações</AppButton>
      <input ref={input} type="file" accept=".json,application/json" hidden onChange={event => void inspect(event)} />
    </div>
    {!native && <p className="hint">Disponível no aplicativo nativo.</p>}
    {busy && !preview && <p role="status">Preparando backup…</p>}
    {message && <p role="status">{message}</p>}
    {error && !preview && <p role="alert">{error}</p>}
    {preview && <Modal title="Importar configurações?" busy={busy} compact onDismiss={() => { setPreview(null); setError(""); }} initialFocus="[data-cancel-import]">
      <p><strong>{preview.name}</strong>: {(preview.config.devices ?? []).length} dispositivo(s) e {preview.config.profiles.length} perfil(is).</p>
      <p>Isso substituirá todos os dispositivos e perfis atuais, incluindo suas ações. Exporte a configuração atual antes de continuar se quiser preservá-la.</p>
      <p className="hint">Se houver remapeamento ativo, as ações importadas poderão ser aplicadas pelo serviço.</p>
      {error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions">
        <AppButton type="button" data-cancel-import disabled={busy} onClick={() => { setPreview(null); setError(""); }}>Cancelar</AppButton>
        <AppButton type="button" className="danger-button" disabled={busy} onClick={() => void restore()}>{busy ? "Importando…" : "Substituir e importar"}</AppButton>
      </div>
    </Modal>}
  </section>;
}
