import { useCallback, useEffect, useState } from "react";
import { CheckCircle2 } from "lucide-react";
import { open as pick } from "@tauri-apps/plugin-dialog";
import { api, CHANGED, type AppInfo, type BambuStatus, type MwCollection } from "./api";
import { fmtDate, fmtDateTime } from "./status";

interface Props {
  info: AppInfo;
  onSaved: (info: AppInfo) => void;
  notify: (t: string) => void;
  fail: (e: unknown) => void;
}

export function SettingsView({ info, onSaved, notify, fail }: Props) {
  const [dataDir, setDataDir] = useState(info.settings.dataDir ?? "");
  const [dbUrl, setDbUrl] = useState(info.settings.databaseUrl ?? "");
  const [studio, setStudio] = useState(info.settings.bambuStudioPath ?? "");
  const [spools, setSpools] = useState(info.settings.spoolsPath ?? "");
  const [busy, setBusy] = useState(false);

  async function save() {
    setBusy(true);
    try {
      await api.saveSettings({ dataDir: dataDir || null, databaseUrl: dbUrl || null, bambuStudioPath: studio || null, spoolsPath: spools || null });
      onSaved(await api.appInfo());
      notify("Einstellungen gespeichert");
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="settings-view">
      <header className="toolbar">
        <h1>Einstellungen</h1>
        <span className="page-sub">Konto, Speicherorte und Verbindung zu Bambu Studio</span>
      </header>

      <BambuAccount notify={notify} fail={fail} />

      <h2 className="settings-group">Speicherorte</h2>

      <section className="section">
        <h3>Datenordner</h3>
        <p className="hint">
          Hier liegen die Datenbank (<code>printmanager.db</code>) und pro Auftrag ein Ordner mit Bildern und Dateien.
          Zum Sichern genügt es, diesen Ordner zu kopieren.
        </p>
        <div className="add-row">
          <input className="grow" value={dataDir} placeholder={info.defaultDataDir} onChange={(e) => setDataDir(e.target.value)} />
          <button
            onClick={async () => {
              const d = await pick({ directory: true, title: "Datenordner wählen" });
              if (typeof d === "string") setDataDir(d);
            }}
          >
            Wählen …
          </button>
        </div>
        <p className="muted small">Aktuell: {info.dataDir}</p>
      </section>

      <section className="section">
        <h3>Datenbank (optional)</h3>
        <p className="hint">
          Leer lassen für SQLite im Datenordner. Für gleichzeitiges Arbeiten auf Mac und Windows kann hier später
          eine PostgreSQL-Datenbank stehen, z.B. <code>postgres://benutzer:passwort@nas/printmanager</code>.
        </p>
        <input value={dbUrl} placeholder="SQLite im Datenordner" onChange={(e) => setDbUrl(e.target.value)} />
        <p className="muted small">Aktuell: {info.database}</p>
      </section>

      <section className="section">
        <h3>Bambu Studio</h3>
        <p className="hint">
          {info.studioFound ? "Bambu Studio wurde gefunden." : "Bambu Studio wurde am Standardort nicht gefunden."} Nur ausfüllen,
          wenn es woanders installiert ist.
        </p>
        <div className="add-row">
          <input className="grow" value={studio} placeholder="Standardort" onChange={(e) => setStudio(e.target.value)} />
          <button
            onClick={async () => {
              const f = await pick({ title: "Bambu Studio wählen" });
              if (typeof f === "string") setStudio(f);
            }}
          >
            Wählen …
          </button>
        </div>
      </section>

      <section className="section">
        <h3>Spulen aus Bambu Studio</h3>
        <p className="hint">
          Die App liest deine Spulen aus <code>spools.json</code> von Bambu Studio und schreibt nie hinein.
          Nur ausfüllen, wenn die Datei nicht am Standardort liegt.
        </p>
        <div className="add-row">
          <input className="grow" value={spools} placeholder={info.defaultSpoolsFile} onChange={(e) => setSpools(e.target.value)} />
          <button
            onClick={async () => {
              const f = await pick({ title: "spools.json wählen", filters: [{ name: "spools.json", extensions: ["json"] }] });
              if (typeof f === "string") setSpools(f);
            }}
          >
            Wählen …
          </button>
        </div>
        <p className="muted small">Aktuell: {info.spoolsFile}</p>
      </section>

      <div className="settings-save">
        <button className="primary" onClick={save} disabled={busy}>Speicherorte übernehmen</button>
        <span className="muted small">Ein neuer Datenordner oder eine andere Datenbank gilt sofort.</span>
      </div>
      <p className="muted small">Printmanager {info.version}</p>
    </div>
  );
}

/** Bambu-Konto: Anmeldung über das Anmeldefenster von Bambu. Die App liest nur. */
function BambuAccount({ notify, fail }: { notify: (t: string) => void; fail: (e: unknown) => void }) {
  const [st, setSt] = useState<BambuStatus | null>(null);
  const load = useCallback(() => api.bambuStatus().then(setSt).catch(fail), [fail]);
  useEffect(() => {
    load();
    const on = () => load();
    window.addEventListener(CHANGED, on);
    // während das Anmeldefenster offen ist, regelmäßig nachsehen
    const t = window.setInterval(load, 3000);
    return () => {
      window.removeEventListener(CHANGED, on);
      window.clearInterval(t);
    };
  }, [load]);
  if (!st) return null;

  const set = (patch: Partial<Pick<BambuStatus, "filament" | "printers" | "makerworld">>) =>
    api.bambuSetOptions({ filament: st.filament, printers: st.printers, makerworld: st.makerworld, ...patch }).then(load).catch(fail);
  const login = () => api.bambuLogin().then(load).catch(fail);

  return (
    <section className="section">
      <h3>Bambu-Konto</h3>
      <p className="hint">
        Für Filament aus dem Konto, Druckerstatus und MakerWorld-Sammlungen. Du meldest dich im Anmeldefenster von
        Bambu Lab an. Die App sieht dein Passwort nicht, legt das Anmelde-Token im Schlüsselbund des Systems ab und
        liest nur. Drucke startest du weiter in Bambu Studio oder Bambu Handy.
      </p>
      {st.connected ? (
        <div className="add-row">
          <span className="grow">
            <span className="ok-badge"><CheckCircle2 size={14} /> Verbunden</span> {st.accountName ?? ""}
            <span className="muted small">
              {st.expiresAt && ` · gültig bis ${fmtDate(st.expiresAt)}`} · Region Global
              {st.lastOkAt && ` · zuletzt erreicht ${fmtDateTime(st.lastOkAt)}`}
            </span>
          </span>
          <button onClick={() => api.bambuLogout().then(() => { notify("Vom Bambu-Konto abgemeldet"); load(); }).catch(fail)}>Abmelden</button>
        </div>
      ) : (
        <div className="add-row">
          <span className="grow">
            {st.expired ? <span className="warn">Anmeldung abgelaufen</span> : <span className="muted">Nicht verbunden</span>}
            {st.loggingIn && <span className="muted small"> · Anmeldefenster ist offen …</span>}
          </span>
          <button className="primary" onClick={login}>{st.expired ? "Neu anmelden" : "Anmelden …"}</button>
        </div>
      )}
      {st.lastError && st.connected && <p className="warn small">{st.lastError}</p>}
      <div className="bambu-opts">
        <label className="check">
          <input type="checkbox" checked={st.filament} onChange={(e) => set({ filament: e.target.checked })} />
          Filament aus dem Konto abgleichen <span className="muted small">(alle 15 Minuten; sonst aus spools.json)</span>
        </label>
        <label className="check">
          <input type="checkbox" checked={st.printers} onChange={(e) => set({ printers: e.target.checked })} />
          Drucker und Druckstatus anzeigen <span className="muted small">(jede Minute, beim Drucken alle 20 s)</span>
        </label>
        <label className="check">
          <input type="checkbox" checked={st.makerworld} onChange={(e) => set({ makerworld: e.target.checked })} />
          MakerWorld-Sammlungen im Katalog <span className="muted small">(alle 6 Stunden; Dateien nur bei Bedarf)</span>
        </label>
        {st.connected && st.makerworld && <MwCollections fail={fail} />}
      </div>
    </section>
  );
}

/** Auswahl, welche MakerWorld-Sammlungen in den Katalog kommen */
function MwCollections({ fail }: { fail: (e: unknown) => void }) {
  const [list, setList] = useState<MwCollection[]>([]);
  const load = useCallback(() => api.mwCollections().then(setList).catch(fail), [fail]);
  useEffect(() => {
    load();
  }, [load]);
  if (!list.length) return <p className="muted small mw-colls">Sammlungen erscheinen nach dem ersten Abgleich (Katalog → „MakerWorld abgleichen“).</p>;
  return (
    <div className="mw-colls">
      {list.map((c) => (
        <label key={c.id} className="check small">
          <input type="checkbox" checked={!!c.enabled} onChange={(e) => api.mwCollectionToggle(c.id, e.target.checked).then(load).catch(fail)} />
          {c.isDefault ? "★ " : ""}{c.title} <span className="muted">({c.designCnt})</span>
        </label>
      ))}
    </div>
  );
}
