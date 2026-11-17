import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Printer as PrinterIcon, X } from "lucide-react";
import {
  api, colorDistance, fmtDuration, fmtGrams, parseColorWish, parseRunFilaments, rankSpools, spoolLabel,
  type OrderDetail, type PrinterView, type PrintRun, type RunFilament, type RunResult, type Spool,
} from "./api";
import { Modal } from "./Modal";
import { PrintPicker } from "./PrintPicker";
import { useAccountWrite } from "./AccountWrite";
import { fmtDateTime } from "./status";

type Run = (fn: () => Promise<unknown>, ok?: string) => Promise<void>;

const RESULTS: { id: RunResult; label: string }[] = [
  { id: "offen", label: "Offen" },
  { id: "ok", label: "Erfolgreich" },
  { id: "fehldruck", label: "Fehldruck" },
];

/** Beste Spule für ein Filament eines Drucks */
function suggest(spools: Spool[], type: string, color: string, grams: number) {
  return rankSpools(spools, { color, material: type || null, grams })[0];
}

/** Drucke eines Auftrags: entstehen, wenn du in Bambu Studio gesliced im Auftragsordner speicherst */
export function PrintRuns({ d, spools, run, notify }: { d: OrderDetail; spools: Spool[]; run: Run; notify: (t: string) => void }) {
  const [deduct, setDeduct] = useState<PrintRun | null>(null);
  const [picking, setPicking] = useState(false);
  const byId = useMemo(() => new Map(spools.map((s) => [s.id, s])), [spools]);
  const write = useAccountWrite(notify);
  /** Nach Abzug oder Rückgängig: geänderte Spulen anbieten, ins Bambu-Konto zu schreiben */
  const offerWrite = async () => {
    if (!write.enabled) return;
    const before = new Map(spools.map((s) => [s.id, s.remainingGrams]));
    const now = await api.spools().catch(() => [] as Spool[]);
    write.ask(
      now
        .filter((s) => before.has(s.id) && Math.abs((before.get(s.id) ?? 0) - s.remainingGrams) > 0.05)
        .map((s) => ({ spool: s, from: s.studioGrams, to: s.remainingGrams })),
    );
  };
  // Drucker, der gerade diesen Auftrag druckt (Bambu-Konto)
  const [live, setLive] = useState<PrinterView | null>(null);
  useEffect(() => {
    const load = () =>
      api.printers().then((r) => setLive(r.printers.find((p) => p.orderId === d.order.id && (p.state === "printing" || p.state === "paused")) ?? null)).catch(() => {});
    load();
    const t = window.setInterval(load, 20000);
    return () => window.clearInterval(t);
  }, [d.order.id]);
  const items = new Map(d.items.map((i) => [i.id, i.title]));

  const scan = () =>
    run(async () => {
      const msgs = await api.orderScan(d.order.id);
      notify(msgs.length ? msgs.join(" · ") : "Keine neue gesliced Datei im Ordner");
    });

  return (
    <section className="section">
      <div className="section-head">
        <h3>Drucke {d.runs.length > 0 && <span className="muted small">{d.runs.length}</span>}</h3>
        <span className="section-actions">
          <button className="small-btn" onClick={() => setPicking(true)} title="Laufenden Druck oder Druck aus dem Druckverlauf diesem Auftrag zuordnen">Druck wählen …</button>
          <button className="ghost small-btn" onClick={scan} title="Auftragsordner nach gesliced 3MF-Dateien durchsuchen">Ordner prüfen</button>
        </span>
      </div>
      {live && (
        <p className="live-line">
          <PrinterIcon size={14} /> {live.state === "paused" ? "Pausiert" : "Druckt"} auf {live.name}
          {live.progress !== null && ` · ${live.progress} %`}
          {live.remainingSeconds ? ` · noch ${fmtDuration(live.remainingSeconds)}` : ""}
        </p>
      )}
      {d.runs.length === 0 && (
        <p className="hint">
          Slice in Bambu Studio und speichere das Projekt (⌘S) im Auftragsordner. Die App erkennt die Datei von selbst,
          übernimmt Druckzeit und Gramm und bietet an, den Verbrauch von deinen Spulen abzuziehen.
        </p>
      )}
      <ul className="runs">
        {d.runs.map((r) => {
          const fils = parseRunFilaments(r.filaments);
          const total = fils.reduce((s, f) => s + f.grams, 0);
          const short = !r.deducted && fils.some((f) => {
            const best = suggest(spools, f.type, f.color, f.grams);
            return best && !best.enough;
          });
          return (
            <li key={r.id} className={`run res-${r.result}`}>
              <div className="run-head">
                <b className="grow" title={r.fileName}>{r.fileName}</b>
                <select
                  value={r.result}
                  onChange={(e) => run(() => api.runSetResult(r.id, e.target.value as RunResult, r.note))}
                  aria-label="Ergebnis"
                >
                  {RESULTS.map((x) => <option key={x.id} value={x.id}>{x.label}</option>)}
                </select>
              </div>
              <div className="run-meta muted small">
                <span>{fmtDateTime(r.createdAt)}</span>
                {r.printSeconds && <span>{fmtDuration(r.printSeconds)}</span>}
                {total > 0 && <span>{fmtGrams(total)}</span>}
                {r.plates > 1 && <span>{r.plates} Platten</span>}
                {r.itemId && items.get(r.itemId) && <span>Teil: {items.get(r.itemId)}</span>}
              </div>
              {r.printerStatus && (
                <div className={`printer-hint ${r.printerStatus === "fehldruck" ? "bad" : ""}`}>
                  <span className="grow">
                    {r.printerName} meldet: <b>{r.printerStatus === "ok" ? "fertig" : "abgebrochen"}</b>
                    {r.actualSeconds ? ` · ${fmtDuration(r.actualSeconds)}` : ""}
                    {actualTotal(r) > 0 ? ` · ${fmtGrams(actualTotal(r))} laut Drucker` : ""}
                  </span>
                  {r.result === "offen" && (
                    <button className="small-btn" onClick={() => run(() => api.runSetResult(r.id, r.printerStatus as RunResult, r.note))}>
                      Als {r.printerStatus === "ok" ? "Erfolgreich" : "Fehldruck"} übernehmen
                    </button>
                  )}
                </div>
              )}
              <ul className="run-fils">
                {fils.map((f, i) => {
                  const sp = f.spoolId ? byId.get(f.spoolId) : undefined;
                  return (
                    <li key={i}>
                      <i className="sw" style={{ background: f.color || "transparent" }} />
                      {f.type || "?"} · {fmtGrams(f.grams)}
                      {r.deducted && sp ? (
                        <span className="muted"> · von {spoolLabel(sp)} (Rest {fmtGrams(sp.remainingGrams)})</span>
                      ) : null}
                    </li>
                  );
                })}
              </ul>
              <div className="run-actions">
                {r.deducted ? (
                  <>
                    <span className="ok-badge"><CheckCircle2 size={14} /> Verbrauch abgezogen</span>
                    <button className="ghost" onClick={() => run(() => api.runUndo(r.id), "Abzug zurückgenommen").then(offerWrite)}>Rückgängig</button>
                  </>
                ) : fils.length > 0 ? (
                  <>
                    <button className="primary" onClick={() => setDeduct(r)}>Verbrauch abziehen …</button>
                    {short && <span className="warn"><AlertTriangle size={13} /> Eine Spule reicht nicht</span>}
                  </>
                ) : (
                  <span className="muted small">Keine Filamentmengen in der Datei</span>
                )}
                <span className="spacer" />
                <button className="ghost icon-btn" title="Druck entfernen (Abzug wird zurückgenommen)" aria-label="Druck entfernen"
                  onClick={() => run(() => api.runDelete(r.id), "Druck entfernt")}><X size={14} /></button>
              </div>
            </li>
          );
        })}
      </ul>
      {picking && (
        <PrintPicker d={d} onClose={() => setPicking(false)} fail={(e) => run(() => Promise.reject(e))}
          onDone={(msg) => { setPicking(false); run(() => Promise.resolve(), msg); }} />
      )}
      {deduct && <DeductDialog r={deduct} spools={spools} onClose={() => setDeduct(null)} onDone={offerWrite} run={run} />}
      {write.dialog}
    </section>
  );
}

function actualTotal(r: PrintRun) {
  return parseRunFilaments(r.actualFilaments || "[]").reduce((s, f) => s + f.grams, 0);
}

/** Echte Gramm laut Drucker den Filamenten des Drucks zuordnen (gleiches Material, ähnlichste Farbe). */
function actualGrams(fils: RunFilament[], actual: RunFilament[]): (number | null)[] {
  const left = [...actual];
  return fils.map((f) => {
    let best = -1;
    let dist = Infinity;
    left.forEach((a, i) => {
      if (a.type.toUpperCase() !== f.type.toUpperCase()) return;
      const dd = colorDistance(a.color, f.color);
      if (dd < dist) {
        dist = dd;
        best = i;
      }
    });
    if (best < 0) return null;
    return left.splice(best, 1)[0].grams;
  });
}

function DeductDialog({ r, spools, onClose, onDone, run }: { r: PrintRun; spools: Spool[]; onClose: () => void; onDone: () => void; run: Run }) {
  const fils = parseRunFilaments(r.filaments);
  const actual = actualGrams(fils, parseRunFilaments(r.actualFilaments || "[]"));
  const hasActual = actual.some((x) => x !== null);
  const [rows, setRows] = useState(() =>
    fils.map((f, i) => {
      const g = actual[i] ?? f.grams;
      return { grams: g, spoolId: suggest(spools, f.type, f.color, g)?.spool.id ?? "" };
    }),
  );
  const byId = new Map(spools.map((s) => [s.id, s]));
  const usable = spools.filter((s) => !s.hidden && !s.missing);

  const save = () =>
    run(
      () => api.runDeduct(r.id, rows.map((x, index) => ({ index, spoolId: x.spoolId || null, grams: x.grams }))),
      "Verbrauch abgezogen",
    ).then(() => { onClose(); onDone(); });

  return (
    <Modal title="Verbrauch abziehen" onClose={onClose}>
      <p className="hint">
        Werte aus <b>{r.fileName}</b>{hasActual ? <> – Gramm <b>laut Drucker</b> ({r.printerName})</> : null}. Vorgeschlagen ist die
        Spule mit passendem Material und der ähnlichsten Farbe. Bei einem Fehldruck kannst du die Gramm anpassen.
      </p>
      <div className="deduct">
        {fils.map((f, i) => {
          const ranked = rankSpools(usable, { color: f.color, material: f.type || null, grams: rows[i].grams });
          const others = usable.filter((s) => !ranked.some((x) => x.spool.id === s.id));
          const sel = byId.get(rows[i].spoolId);
          const set = (patch: Partial<(typeof rows)[number]>) => setRows((all) => all.map((x, j) => (j === i ? { ...x, ...patch } : x)));
          return (
            <div key={i} className="deduct-row">
              <i className="sw big" style={{ background: f.color || "transparent" }} title={f.color} />
              <span className="deduct-type">{f.type || "?"}</span>
              <input type="number" min={0} step={0.1} className="qty" value={rows[i].grams}
                onChange={(e) => set({ grams: Number(e.target.value) })} aria-label="Gramm" />
              <span className="muted">g von</span>
              <select className="grow" value={rows[i].spoolId} onChange={(e) => set({ spoolId: e.target.value })} aria-label="Spule">
                <option value="">– nicht abziehen –</option>
                {ranked.length > 0 && (
                  <optgroup label="Passend">
                    {ranked.map(({ spool: s }) => (
                      <option key={s.id} value={s.id}>{spoolLabel(s)} · {s.color} · {fmtGrams(s.remainingGrams)}</option>
                    ))}
                  </optgroup>
                )}
                {others.length > 0 && (
                  <optgroup label="Andere">
                    {others.map((s) => (
                      <option key={s.id} value={s.id}>{s.material} {spoolLabel(s)} · {s.color} · {fmtGrams(s.remainingGrams)}</option>
                    ))}
                  </optgroup>
                )}
              </select>
              {sel && <i className="sw big" style={{ background: sel.color }} title={`Spule ${sel.color}`} />}
              {sel && sel.remainingGrams < rows[i].grams && (
                <div className="warn full"><AlertTriangle size={13} /> Auf dieser Spule sind nur noch {fmtGrams(sel.remainingGrams)} – das reicht nicht.</div>
              )}
            </div>
          );
        })}
      </div>
      {spools.length === 0 && <p className="warn">Noch keine Spulen eingelesen (Seite „Filament“).</p>}
      <footer className="row-end">
        <button onClick={onClose}>Abbrechen</button>
        <button className="primary" onClick={save}>Abziehen</button>
      </footer>
    </Modal>
  );
}

/** Kleiner Farbpunkt am Teil: passende Spule zum Farbwunsch */
export function SpoolDot({ wish, spools }: { wish: string; spools: Spool[] }) {
  if (!wish.trim() || spools.length === 0) return <span className="spool-dot none" aria-hidden />;
  const want = parseColorWish(wish);
  if (!want.color && !want.material) return <span className="spool-dot none" title="Farbwunsch nicht erkannt" />;
  const best = rankSpools(spools, want).filter((x) => !want.color || x.distance < 60);
  if (!best.length) return <span className="spool-dot missing" title={`Keine passende Spule für „${wish}“`}>!</span>;
  const s = best[0].spool;
  const more = best.length > 1 ? ` (+${best.length - 1} weitere)` : "";
  return (
    <span className="spool-dot" style={{ background: s.color }}
      title={`Passende Spule: ${s.material} ${spoolLabel(s)} · ${fmtGrams(s.remainingGrams)} übrig${more}`} />
  );
}
