import { useCallback, useEffect, useState } from "react";
import { CloudUpload } from "lucide-react";
import { api, fmtGrams, spoolLabel, type Spool, type WriteResult } from "./api";
import { Modal } from "./Modal";

/** Eine Rest-Änderung, die ins Bambu-Konto soll */
export interface RestChange { spool: Spool; from: number; to: number }

type Row = RestChange & { result?: WriteResult; busy?: boolean };

/**
 * Nach einer Rest-Änderung in der App fragen, ob sie ins Bambu-Konto soll (nur wenn der Schalter an ist).
 * `ask(changes)` öffnet den Dialog; `dialog` muss gerendert werden.
 */
export function useAccountWrite(notify: (t: string, error?: boolean) => void) {
  const [enabled, setEnabled] = useState(false);
  const [rows, setRows] = useState<Row[] | null>(null);

  useEffect(() => {
    api.bambuStatus().then((b) => setEnabled(b.connected && b.filamentWrite)).catch(() => setEnabled(false));
  }, []);

  const ask = useCallback(
    (changes: RestChange[]) => {
      const list = changes.filter((c) => c.spool.source === "konto" && Math.round(c.from) !== Math.round(c.to));
      if (enabled && list.length) setRows(list);
    },
    [enabled],
  );

  const write = async (i: number, force = false) => {
    setRows((r) => r && r.map((x, j) => (j === i ? { ...x, busy: true } : x)));
    const row = rows![i];
    try {
      const result = await api.spoolPushRest(row.spool.id, row.to, force);
      setRows((r) => r && r.map((x, j) => (j === i ? { ...x, busy: false, result } : x)));
      return result;
    } catch (e) {
      const result: WriteResult = { status: "fehler", message: String(e), cloudGrams: null };
      setRows((r) => r && r.map((x, j) => (j === i ? { ...x, busy: false, result } : x)));
      return result;
    }
  };

  const writeAll = async () => {
    if (!rows) return;
    let ok = 0;
    for (let i = 0; i < rows.length; i++) {
      if (!rows[i].result) {
        const r = await write(i);
        if (r.status === "ok") ok++;
      }
    }
    if (ok) notify(ok === 1 ? "Rest im Bambu-Konto gespeichert" : `${ok} Spulen im Bambu-Konto gespeichert`);
  };

  // Konflikt: Wert aus dem Konto in die App übernehmen
  const takeCloud = async (i: number) => {
    const row = rows![i];
    const g = row.result?.cloudGrams;
    if (g == null) return;
    await api.spoolUpdate({ id: row.spool.id, remainingGrams: g, hidden: row.spool.hidden, note: row.spool.note });
    setRows((r) => r && r.map((x, j) => (j === i ? { ...x, result: { status: "ok", message: `Wert aus dem Konto übernommen: ${fmtGrams(g)}`, cloudGrams: g } } : x)));
  };

  const done = rows?.every((r) => r.result && r.result.status !== "konflikt");
  const dialog = rows && (
    <Modal title="Ins Bambu-Konto übernehmen?" onClose={() => setRows(null)}>
      <p className="hint">
        Der Rest hat sich in Printmanager geändert. Soll er auch im Bambu-Konto stehen (Bambu Studio und Handy)?
        Bambu-Spulen mit RFID pflegt das AMS. Dort schreibt die App nichts.
      </p>
      <ul className="push-list">
        {rows.map((r, i) => (
          <li key={r.spool.id} className={`push-row ${r.result?.status ?? ""}`}>
            <i className="sw big" style={{ background: r.spool.color || "transparent" }} />
            <span className="grow">
              <b>{spoolLabel(r.spool)}</b>
              <span className="muted small"> · {fmtGrams(r.from)} → <b>{fmtGrams(r.to)}</b>{r.spool.rfid ? " · RFID" : ""}</span>
              {r.result && <span className={`push-msg ${r.result.status}`}>{r.result.message}</span>}
            </span>
            {r.result?.status === "konflikt" && (
              <span className="push-actions">
                <button className="small-btn" onClick={() => takeCloud(i)}>Konto-Wert übernehmen ({fmtGrams(r.result.cloudGrams ?? 0)})</button>
                <button className="small-btn" onClick={() => write(i, true)}>Trotzdem schreiben</button>
              </span>
            )}
            {r.busy && <span className="muted small">schreibt …</span>}
          </li>
        ))}
      </ul>
      <footer className="row-end">
        {done ? (
          <button className="primary" onClick={() => setRows(null)}>Fertig</button>
        ) : (
          <>
            <button onClick={() => setRows(null)}>Nur in der App</button>
            <button className="primary" onClick={writeAll} disabled={rows.some((r) => r.busy)}><CloudUpload size={14} /> Ins Konto schreiben</button>
          </>
        )}
      </footer>
    </Modal>
  );
  return { enabled, ask, dialog };
}
