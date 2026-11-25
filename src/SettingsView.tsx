import { useEffect, useState } from "react";
import { open as pick } from "@tauri-apps/plugin-dialog";
import { api, type AppInfo } from "./api";
import { calc, eur, pricingApi, type Pricing } from "./pricing";
import { ConnectorsSection } from "./ConnectorsSection";

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
        <span className="page-sub">Anbindungen, Kalkulation und Speicherorte</span>
      </header>

      <ConnectorsSection notify={notify} fail={fail} />

      <CalcSettings notify={notify} fail={fail} />

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

/** Werte für Kosten und Verkaufspreis */
function CalcSettings({ notify, fail }: { notify: (t: string) => void; fail: (e: unknown) => void }) {
  const [p, setP] = useState<Pricing | null>(null);
  const [newMat, setNewMat] = useState("");
  useEffect(() => {
    pricingApi.get().then(setP).catch(fail);
  }, [fail]);
  if (!p) return null;
  const num = (k: keyof Pricing, label: string, unit: string, step = 0.01) => (
    <label className="block">
      {label} ({unit})
      <input type="number" min={0} step={step} value={p[k] as number} onChange={(e) => setP({ ...p, [k]: Number(e.target.value) })} />
    </label>
  );
  const mats = Object.entries(p.filamentEurPerKg).sort(([a], [b]) => (a === "default" ? -1 : b === "default" ? 1 : a.localeCompare(b)));
  // Beispiel: 100 g PLA, 3 h
  const ex = calc(p, { basis: "slicer", filaments: [{ type: "PLA", grams: 100 }], seconds: 3 * 3600 });
  return (
    <section className="section">
      <h3>Kalkulation</h3>
      <p className="hint">
        Daraus berechnet die App im Auftrag Kosten und einen Preisvorschlag, zuerst geschätzt (Katalog oder Slicer),
        nach dem Druck mit den tatsächlichen Werten. Kosten = (Material + Strom + Maschine) × (1 + Puffer) + Pauschale;
        Preis = (Kosten ohne Pauschale) × Faktor + Pauschale, aufgerundet.
      </p>
      <div className="calc-grid">
        {num("powerWatts", "Leistung Drucker", "W", 1)}
        {num("powerEurPerKwh", "Strompreis", "€/kWh")}
        {num("machineEurPerHour", "Maschine/Verschleiß", "€/h")}
        {num("failurePct", "Fehldruck-Puffer", "%", 1)}
        {num("handlingEur", "Pauschale je Auftrag", "€", 0.5)}
        {num("markupFactor", "Preisfaktor", "×", 0.1)}
        {num("minPriceEur", "Mindestpreis", "€", 0.5)}
        {num("roundToEur", "Aufrunden auf", "€", 0.1)}
      </div>
      <h4 className="calc-sub">Filamentpreise (€/kg)</h4>
      <div className="calc-mats">
        {mats.map(([m, v]) => (
          <label key={m} className="block">
            {m === "default" ? "Andere" : m}
            <span className="add-row">
              <input type="number" min={0} step={0.5} value={v} onChange={(e) => setP({ ...p, filamentEurPerKg: { ...p.filamentEurPerKg, [m]: Number(e.target.value) } })} />
              {m !== "default" && (
                <button className="icon-btn" aria-label={`${m} entfernen`} onClick={() => {
                  const next = { ...p.filamentEurPerKg };
                  delete next[m];
                  setP({ ...p, filamentEurPerKg: next });
                }}>×</button>
              )}
            </span>
          </label>
        ))}
        <label className="block">
          Material hinzufügen
          <span className="add-row">
            <input value={newMat} placeholder="z.B. PA-CF" onChange={(e) => setNewMat(e.target.value)} />
            <button disabled={!newMat.trim()} onClick={() => { setP({ ...p, filamentEurPerKg: { ...p.filamentEurPerKg, [newMat.trim().toUpperCase()]: 30 } }); setNewMat(""); }}>+</button>
          </span>
        </label>
      </div>
      <div className="settings-save calc-foot">
        <button className="primary" onClick={() => pricingApi.save(p).then(() => notify("Kalkulation gespeichert")).catch(fail)}>Kalkulation speichern</button>
        <span className="muted small">Beispiel 100 g PLA, 3 h: Kosten {eur(ex.cost)}, Preis {eur(ex.price)}</span>
      </div>
    </section>
  );
}
