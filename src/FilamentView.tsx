import { useCallback, useEffect, useMemo, useState } from "react";
import { Eye, EyeOff, Printer as PrinterIcon, RefreshCw, Search } from "lucide-react";
import { api, fmtGrams, spoolLabel, type AppInfo, type BambuStatus, type Spool } from "./api";
import { fmtDateTime } from "./status";

interface Props {
  info: AppInfo;
  rev: number;
  notify: (t: string, error?: boolean) => void;
  fail: (e: unknown) => void;
}

export const LOW = 100; // ab hier gilt eine Spule als knapp

export function FilamentView({ info, rev, notify, fail }: Props) {
  const [all, setAll] = useState<Spool[]>([]);
  const [q, setQ] = useState("");
  const [showAll, setShowAll] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [bambu, setBambu] = useState<BambuStatus | null>(null);
  const fromAccount = !!bambu && bambu.connected && bambu.filament;

  const load = useCallback(
    () => api.spools().then((l) => { setAll(l); setLoaded(true); }).catch(fail),
    [fail],
  );
  useEffect(() => {
    load();
    api.bambuStatus().then(setBambu).catch(() => setBambu(null));
  }, [load, rev]);

  // Beim Öffnen der Seite abgleichen (das Konto höchstens alle 2 Minuten)
  useEffect(() => {
    api.filamentSync(false).catch(() => {});
  }, []);

  const sync = async () => {
    try {
      const r = await api.filamentSync(true);
      const where = r.source === "konto" ? "im Bambu-Konto" : "in Bambu Studio";
      if (!r.found) notify(`spools.json nicht gefunden: ${r.path}`, true);
      else notify(`Abgeglichen: ${r.added} neu, ${r.updated} ${where} geändert${r.missing ? `, ${r.missing} nicht mehr vorhanden` : ""}`);
      load();
    } catch (e) {
      fail(e);
    }
  };

  const groups = useMemo(() => {
    const s = q.trim().toLowerCase();
    const list = all
      .filter((x) => showAll || (!x.hidden && !x.missing && x.remainingGrams > 0))
      .filter((x) => !s || `${x.material} ${x.brand} ${x.series} ${x.colorName} ${x.color} ${x.note}`.toLowerCase().includes(s));
    const map = new Map<string, Spool[]>();
    for (const x of list) {
      const k = x.material || "Unbekannt";
      map.set(k, [...(map.get(k) ?? []), x]);
    }
    for (const l of map.values()) l.sort((a, b) => b.inPrinter - a.inPrinter);
    return [...map.entries()].sort((a, b) => b[1].length - a[1].length);
  }, [all, q, showAll]);

  const hiddenCount = all.filter((x) => x.hidden || x.missing || x.remainingGrams <= 0).length;

  return (
    <div className="filament-view">
      <header className="toolbar">
        <h1>Filament</h1>
        <label className="search"><Search size={14} /><input type="search" placeholder="Material, Marke, Farbe …" value={q} onChange={(e) => setQ(e.target.value)} /></label>
        <label className="check">
          <input type="checkbox" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} /> Leere & ausgeblendete ({hiddenCount})
        </label>
        <button className="primary" onClick={sync}><RefreshCw size={13} /> Jetzt abgleichen</button>
      </header>
      {fromAccount ? (
        <p className="hint pad">
          Deine Spulen aus dem <b>Bambu-Konto</b>, abgeglichen alle 15 Minuten
          {bambu?.filamentSyncedAt ? `, zuletzt ${fmtDateTime(bambu.filamentSyncedAt)}` : ""}. Die App zieht den Verbrauch deiner
          Drucke hier ab und ändert im Konto nichts. Änderst du dort (Bambu Studio oder Handy) den Rest einer Spule, gilt der neue Wert.
        </p>
      ) : (
        <p className="hint pad">
          Deine Spulen aus Bambu Studio. Die App zieht den Verbrauch deiner Drucke hier ab und ändert in Bambu Studio nichts.
          Änderst du dort den Rest einer Spule, übernimmt die App den neuen Wert. Quelle: <code>{info.spoolsFile}</code>.
          Aktueller wird es mit dem Bambu-Konto (Einstellungen).
        </p>
      )}

      {loaded && all.length === 0 && (
        <div className="empty pad">
          Noch keine Spulen. Lege sie in Bambu Studio unter „Filament“ an und klicke dann auf „Jetzt abgleichen“.
        </div>
      )}

      {groups.map(([material, list]) => (
        <section key={material} className="spool-group">
          <h2>
            {material} <span className="muted small">{list.length} Spulen · {fmtGrams(list.reduce((s, x) => s + x.remainingGrams, 0))}</span>
          </h2>
          <div className="spool-grid">
            {list.map((s) => <SpoolCard key={`${s.id}-${s.updatedAt}`} s={s} notify={notify} fail={fail} />)}
          </div>
        </section>
      ))}
    </div>
  );
}

function SpoolCard({ s, notify, fail }: { s: Spool; notify: (t: string) => void; fail: (e: unknown) => void }) {
  const pct = s.initialGrams > 0 ? Math.min(100, (s.remainingGrams / s.initialGrams) * 100) : 0;
  const low = s.remainingGrams < LOW;
  const save = (patch: Partial<{ remainingGrams: number; hidden: number }>) =>
    api.spoolUpdate({ id: s.id, remainingGrams: s.remainingGrams, hidden: s.hidden, note: s.note, ...patch }).catch(fail);

  return (
    <article className={`spool ${s.hidden || s.missing ? "off" : ""}`}>
      <div className="spool-top">
        <i className="sw spool-color" style={{ background: s.color || "transparent" }} title={s.color} />
        <div className="spool-name">
          <div className="title">{spoolLabel(s)}</div>
          <div className="muted small">{s.brand} · {s.color}</div>
          {s.inPrinter ? (
            <div className="in-printer small" title="Laut Bambu-Konto gerade im Drucker"><PrinterIcon size={12} /> {s.printerName}{s.amsSlot && ` · ${s.amsSlot}`}</div>
          ) : null}
        </div>
      </div>
      <div className="bar" title={`${Math.round(pct)} % übrig`}>
        <span style={{ width: `${pct}%`, background: low ? "var(--late)" : s.color || "var(--accent)" }} />
      </div>
      <div className="spool-rest">
        <input
          type="number"
          min={0}
          step={1}
          defaultValue={Math.round(s.remainingGrams * 10) / 10}
          aria-label="Rest in Gramm"
          onBlur={(e) => {
            const v = Number(e.target.value);
            if (Number.isFinite(v) && Math.abs(v - s.remainingGrams) > 0.05) save({ remainingGrams: v }).then(() => notify("Rest gespeichert"));
          }}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
        <span className="muted small">von {fmtGrams(s.initialGrams)}</span>
        <span className="spacer" />
        {low && <span className="warn small">knapp</span>}
      </div>
      <div className="spool-foot">
        {s.missing ? <span className="muted small">{s.source === "konto" ? "nicht mehr im Bambu-Konto" : "nicht mehr in Bambu Studio"}</span> : null}
        {s.missing ? null : s.depleted ? <span className="warn small">laut Bambu leer</span> : null}
        {Math.abs(s.remainingGrams - s.studioGrams) > 0.5 && !s.missing && (
          <span className="muted small" title="Bambu kennt den Verbrauch aus der App nicht">
            {s.source === "konto" ? "Konto" : "Studio"}: {fmtGrams(s.studioGrams)}
          </span>
        )}
        <span className="spacer" />
        <button className="ghost small-btn" onClick={() => save({ hidden: s.hidden ? 0 : 1 })}>
          {s.hidden ? <><Eye size={12} /> Einblenden</> : <><EyeOff size={12} /> Ausblenden</>}
        </button>
      </div>
    </article>
  );
}
