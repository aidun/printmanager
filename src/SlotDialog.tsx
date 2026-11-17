import { useEffect, useState } from "react";
import { api, spoolLabel, type PrinterView, type Spool, type WriteResult } from "./api";
import { Modal } from "./Modal";

const EXTERN = 254;

/**
 * Spule im Bambu-Konto einem Drucker und AMS-Slot zuordnen.
 * Ändert nur den Eintrag im Konto, nicht den Drucker; ein AMS-Abgleich von Bambu kann ihn überschreiben.
 */
export function SlotDialog({ spool, onClose, notify }: { spool: Spool; onClose: () => void; notify: (t: string, error?: boolean) => void }) {
  const [printers, setPrinters] = useState<PrinterView[]>([]);
  const [printer, setPrinter] = useState("");
  const [ams, setAms] = useState(0);
  const [slot, setSlot] = useState(0);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<WriteResult | null>(null);

  useEffect(() => {
    api.printers().then((r) => {
      setPrinters(r.printers);
      const cur = r.printers.find((p) => p.name.toLowerCase() === spool.printerName.toLowerCase());
      setPrinter((cur ?? r.printers[0])?.id ?? "");
    }).catch(() => setPrinters([]));
  }, [spool.printerName]);

  const go = async (assign: boolean) => {
    setBusy(true);
    try {
      const r = await api.spoolAssignSlot(spool.id, assign ? printer : null, ams, ams === EXTERN ? 0 : slot);
      setResult(r);
      if (r.status === "ok") notify(r.message);
    } catch (e) {
      setResult({ status: "fehler", message: String(e), cloudGrams: null });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Drucker und Slot zuordnen" onClose={onClose}>
      <p>
        <b>{spoolLabel(spool)}</b>
        <span className="muted small"> · jetzt: {spool.inPrinter ? `${spool.printerName} · ${spool.amsSlot}` : "in keinem Drucker"}</span>
      </p>
      <p className="hint">
        Trägt die Zuordnung im Bambu-Konto ein, damit Bambu Studio, Handy und Printmanager sie zeigen. Der Drucker selbst
        bekommt nichts davon mit. Gleicht Bambu das AMS ab, gilt wieder dessen Stand.
      </p>
      {printers.length === 0 ? (
        <p className="warn">Keine Drucker aus dem Bambu-Konto (Einstellungen → „Drucker und Druckstatus anzeigen“).</p>
      ) : (
        <div className="slot-form">
          <label>Drucker
            <select value={printer} onChange={(e) => setPrinter(e.target.value)}>
              {printers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </label>
          <label>AMS
            <select value={ams} onChange={(e) => setAms(Number(e.target.value))}>
              {[0, 1, 2, 3].map((a) => <option key={a} value={a}>AMS {a + 1}</option>)}
              <option value={EXTERN}>Externe Spule</option>
            </select>
          </label>
          <label>Slot
            <select value={slot} disabled={ams === EXTERN} onChange={(e) => setSlot(Number(e.target.value))}>
              {[0, 1, 2, 3].map((s) => <option key={s} value={s}>Slot {s + 1}</option>)}
            </select>
          </label>
        </div>
      )}
      {result && <p className={`push-msg ${result.status}`}>{result.message}</p>}
      <footer className="row-end">
        {spool.inPrinter ? <button onClick={() => go(false)} disabled={busy}>Aus dem Drucker nehmen</button> : null}
        <span className="spacer" />
        <button onClick={onClose}>{result?.status === "ok" ? "Fertig" : "Abbrechen"}</button>
        <button className="primary" onClick={() => go(true)} disabled={busy || !printer}>{busy ? "Schreibt …" : "Im Konto zuordnen"}</button>
      </footer>
    </Modal>
  );
}
