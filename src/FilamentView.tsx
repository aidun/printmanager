import { useCallback, useEffect, useMemo, useState } from "react";
import { Eye, EyeOff, Minus, Plus, Printer as PrinterIcon, RefreshCw, Search, Trash2 } from "lucide-react";
import { useAccountWrite, type RestChange } from "./AccountWrite";
import { Modal } from "./Modal";
import { SlotDialog } from "./SlotDialog";
import { NewSpoolDialog } from "./NewSpoolDialog";
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
  const write = useAccountWrite(notify);
  /** Vorrat ändern: Bestätigung, welche Spule angelegt bzw. gelöscht wird */
  const [stock, setStock] = useState<{ kind: "add" | "remove"; spool: Spool; count: number } | null>(null);
  const [stockBusy, setStockBusy] = useState(false);
  const [slotFor, setSlotFor] = useState<Spool | null>(null);
  const [creating, setCreating] = useState(false);

  /** gleichartige Spulen (Marke, Produkt, Material, Farbe) im Konto */
  const variant = (x: Spool) => `${x.brand}|${x.series}|${x.material}|${x.color}`.toLowerCase();
  const variants = useMemo(() => {
    const m = new Map<string, Spool[]>();
    for (const x of all) if (!x.missing && x.source === "konto") m.set(variant(x), [...(m.get(variant(x)) ?? []), x]);
    return m;
  }, [all]);
  /** Welche Spule beim Verringern gelöscht wird: leere zuerst, dann die mit dem kleinsten Rest; nie im Drucker, nie RFID */
  const removable = (key: string) =>
    (variants.get(key) ?? [])
      .filter((x) => !x.inPrinter && !x.rfid)
      .sort((a, b) => b.depleted - a.depleted || a.remainingGrams - b.remainingGrams)[0];
  const stockGo = async () => {
    if (!stock) return;
    setStockBusy(true);
    try {
      const r = stock.kind === "add" ? await api.spoolStockAdd(stock.spool.id) : await api.spoolStockRemove(stock.spool.id);
      notify(r.message, r.status !== "ok");
      setStock(null);
      load();
    } catch (e) {
      fail(e);
    } finally {
      setStockBusy(false);
    }
  };

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
        {write.enabled && fromAccount && <button onClick={() => setCreating(true)}><Plus size={14} /> Spule</button>}
        <button className="primary" onClick={sync}><RefreshCw size={13} /> Abgleichen</button>
      </header>
      {fromAccount ? (
        <p className="hint pad">
          Deine Spulen aus dem <b>Bambu-Konto</b>, abgeglichen alle 15 Minuten
          {bambu?.filamentSyncedAt ? `, zuletzt ${fmtDateTime(bambu.filamentSyncedAt)}` : ""}. Die App zieht den Verbrauch deiner
          Drucke hier ab{write.enabled
            ? " und fragt danach, ob der neue Rest ins Konto soll. Mit „+ Spule“ legst du Spulen im Konto an, mit − und + änderst du den Vorrat gleicher Spulen, der Papierkorb löscht eine Spule."
            : " und ändert im Konto nichts."}{" "}
          Änderst du dort (Bambu Studio oder Handy) den Rest einer Spule, gilt der neue Wert.
        </p>
      ) : (
        <p className="hint pad">
          Deine Spulen aus Bambu Studio. Die App zieht den Verbrauch deiner Drucke hier ab und ändert in Bambu Studio nichts.
          Änderst du dort den Rest einer Spule, übernimmt die App den neuen Wert. Quelle: <code>{info.spoolsFile}</code>.
          Aktueller wird es mit dem Bambu-Konto (Einstellungen → Anbindungen).
        </p>
      )}

      {loaded && all.length === 0 && (
        <div className="empty pad">
          Noch keine Spulen. Lege sie in Bambu Studio unter „Filament“ an und klicke dann auf „Abgleichen“.
        </div>
      )}

      {groups.map(([material, list]) => (
        <section key={material} className="spool-group">
          <h2>
            {material} <span className="muted small">{list.length} Spulen · {fmtGrams(list.reduce((s, x) => s + x.remainingGrams, 0))}</span>
          </h2>
          <div className="spool-grid">
            {list.map((s) => {
              const key = variant(s);
              const count = variants.get(key)?.length ?? 0;
              const canStock = write.enabled && s.source === "konto" && !s.missing;
              const cand = removable(key);
              return (
                <SpoolCard key={`${s.id}-${s.updatedAt}`} s={s} notify={notify} fail={fail}
                  onRest={(c) => write.ask([c])}
                  onSlot={canStock && !s.rfid ? () => setSlotFor(s) : undefined}
                  onDelete={canStock && !s.rfid && !s.inPrinter ? () => setStock({ kind: "remove", spool: s, count }) : undefined}
                  stock={canStock ? {
                    count,
                    add: () => setStock({ kind: "add", spool: s, count }),
                    remove: cand ? () => setStock({ kind: "remove", spool: cand, count }) : undefined,
                  } : undefined} />
              );
            })}
          </div>
        </section>
      ))}
      {write.dialog}
      {creating && <NewSpoolDialog spools={all} notify={notify} onClose={() => { setCreating(false); load(); }} />}
      {slotFor && <SlotDialog spool={slotFor} notify={notify} onClose={() => { setSlotFor(null); load(); }} />}
      {stock && (
        <Modal title={stock.kind === "add" ? "Spule hinzufügen" : "Spule entfernen"} onClose={() => setStock(null)}>
          <p>
            {stock.kind === "add" ? (
              <>Eine neue, volle Spule <b>{spoolLabel(stock.spool)}</b> ({fmtGrams(stock.spool.initialGrams)}) im Bambu-Konto anlegen?</>
            ) : (
              <>Diese Spule im Bambu-Konto <b>löschen</b>: <b>{spoolLabel(stock.spool)}</b> mit {fmtGrams(stock.spool.remainingGrams)} Rest{stock.spool.depleted ? " (leer)" : ""}?</>
            )}
          </p>
          <p className="hint">Vorrat dieser Sorte: {stock.count} → {stock.kind === "add" ? stock.count + 1 : stock.count - 1}. Erscheint danach auch in Bambu Studio und Handy.</p>
          <footer className="row-end">
            <button onClick={() => setStock(null)}>Abbrechen</button>
            <button className={stock.kind === "add" ? "primary" : "danger"} onClick={stockGo} disabled={stockBusy}>
              {stockBusy ? "Schreibt …" : stock.kind === "add" ? "Anlegen" : "Löschen"}
            </button>
          </footer>
        </Modal>
      )}
    </div>
  );
}

function SpoolCard({
  s, notify, fail, onRest, onSlot, onDelete, stock,
}: {
  s: Spool; notify: (t: string) => void; fail: (e: unknown) => void; onRest: (c: RestChange) => void; onSlot?: () => void; onDelete?: () => void;
  stock?: { count: number; add: () => void; remove?: () => void };
}) {
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
            if (Number.isFinite(v) && Math.abs(v - s.remainingGrams) > 0.05) {
              save({ remainingGrams: v }).then(() => {
                notify("Rest gespeichert");
                onRest({ spool: s, from: s.studioGrams, to: v });
              });
            }
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
        {onSlot && (
          <button className="icon-btn" onClick={onSlot} aria-label="Drucker und Slot zuordnen" title="Drucker und AMS-Slot im Konto zuordnen"><PrinterIcon size={13} /></button>
        )}
        {onDelete && (
          <button className="icon-btn" onClick={onDelete} aria-label="Spule löschen" title="Diese Spule im Bambu-Konto löschen"><Trash2 size={13} /></button>
        )}
        {stock && (
          <span className="stock" title="Gleiche Spulen (Marke, Produkt, Farbe) im Bambu-Konto">
            <button className="icon-btn" onClick={stock.remove} disabled={!stock.remove} aria-label="Eine Spule weniger" title={stock.remove ? "Eine Spule aus dem Konto löschen" : "Keine löschbare Spule (im Drucker oder RFID)"}><Minus size={12} /></button>
            <span className="stock-n">{stock.count}×</span>
            <button className="icon-btn" onClick={stock.add} aria-label="Eine Spule mehr" title="Volle gleiche Spule im Konto anlegen"><Plus size={12} /></button>
          </span>
        )}
        <button className="icon-btn" onClick={() => save({ hidden: s.hidden ? 0 : 1 })}
          aria-label={s.hidden ? "Einblenden" : "Ausblenden"} title={s.hidden ? "Wieder anzeigen" : "Ausblenden (leer oder verliehen)"}>
          {s.hidden ? <Eye size={13} /> : <EyeOff size={13} />}
        </button>
      </div>
    </article>
  );
}
