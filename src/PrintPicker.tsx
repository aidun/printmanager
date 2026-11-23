import { useEffect, useMemo, useState } from "react";
import { Printer, Search } from "lucide-react";
import {
  api, fmtDuration, fmtGrams, fmtProgress, historyGrams, historySeconds, parseRunFilaments,
  type HistoryEntry, type OrderDetail, type PrinterView,
} from "./api";
import { Modal } from "./Modal";
import { fmtDateTime } from "./status";

type Period = "7" | "30" | "90" | "alle";

/**
 * Im Auftrag einen laufenden Druck (Drucker-Seite) oder einen vergangenen Druck (Druckverlauf) wählen.
 * Laufend: Der Druckauftrag wird diesem Auftrag fest zugeordnet. Vergangen: Dauer und Gramm kommen an den Druck im Auftrag.
 */
export function PrintPicker({ d, onClose, onDone, fail }: { d: OrderDetail; onClose: () => void; onDone: (msg: string) => void; fail: (e: unknown) => void }) {
  const [printers, setPrinters] = useState<PrinterView[]>([]);
  const [history, setHistory] = useState<HistoryEntry[] | null>(null);
  const [q, setQ] = useState("");
  const [onlyFree, setOnlyFree] = useState(true);
  const [period, setPeriod] = useState<Period>("30");

  useEffect(() => {
    api.printers().then((r) => setPrinters(r.printers.filter((p) => (p.state === "printing" || p.state === "paused") && p.taskName))).catch(() => {});
    api.history().then(setHistory).catch(fail);
  }, [fail]);

  const rows = useMemo(() => {
    const s = q.trim().toLowerCase();
    return (history ?? [])
      .filter((h) => !onlyFree || !h.orderId || h.orderId === d.order.id)
      .filter((h) => {
        if (period === "alle") return true;
        const t = h.endTime ?? h.startTime;
        return !!t && Date.now() - new Date(t).getTime() <= Number(period) * 86400000;
      })
      .filter((h) => !s || `${h.title} ${h.designTitle} ${h.deviceName}`.toLowerCase().includes(s));
  }, [history, q, onlyFree, period, d.order.id]);

  const pickRunning = (p: PrinterView) =>
    api.printerAssign(p.taskName!, d.order.id).then(() => onDone(`Laufender Druck auf ${p.name} gehört jetzt zu #${d.order.number}`)).catch(fail);
  const pickPast = (h: HistoryEntry) =>
    api.historyAssign(h.id, d.order.id).then(() => onDone("Druck zugeordnet – Dauer und Gramm stehen jetzt unter „Drucke“")).catch(fail);

  return (
    <Modal title={`Druck für #${d.order.number} wählen`} onClose={onClose} wide>
      {printers.length > 0 && (
        <section className="pp-section">
          <h3>Läuft gerade</h3>
          <ul className="pp-list">
            {printers.map((p) => (
              <li key={p.id}>
                <button className="pp-row" onClick={() => pickRunning(p)}>
                  <Printer size={16} />
                  <span className="grow">
                    <span className="h-title">{p.taskName}</span>
                    <span className="muted small">{p.name}{p.progress !== null ? ` · ${fmtProgress(p)}` : ""}{p.remainingSeconds ? ` · noch ${fmtDuration(p.remainingSeconds)}` : ""}</span>
                  </span>
                  {p.orderId && p.orderId !== d.order.id && <span className="muted small">jetzt #{p.orderNumber}</span>}
                  {p.orderId === d.order.id && <span className="ok-badge small">schon zugeordnet</span>}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="pp-section">
        <h3>Druckverlauf</h3>
        <div className="filters">
          <label className="search grow-search">
            <Search size={13} />
            <input autoFocus value={q} placeholder="Titel, Modell, Drucker …" onChange={(e) => setQ(e.target.value)} />
          </label>
          <div className="seg" role="group" aria-label="Zeitraum">
            {([["7", "7 Tage"], ["30", "30 Tage"], ["90", "90 Tage"], ["alle", "Alles"]] as const).map(([id, label]) => (
              <button key={id} className={period === id ? "active" : ""} onClick={() => setPeriod(id)}>{label}</button>
            ))}
          </div>
          <label className="check small"><input type="checkbox" checked={onlyFree} onChange={(e) => setOnlyFree(e.target.checked)} /> nur ohne Auftrag</label>
        </div>
        {history === null ? (
          <p className="muted">Lädt …</p>
        ) : rows.length === 0 ? (
          <p className="muted small">{history.length ? "Kein Druck passt zu den Filtern." : "Noch kein Druckverlauf – dafür braucht es das Bambu-Konto."}</p>
        ) : (
          <ul className="pp-list scroll">
            {rows.map((h) => {
              const g = historyGrams(h);
              const secs = historySeconds(h);
              const own = h.orderId === d.order.id;
              return (
                <li key={h.id}>
                  <button className="pp-row" onClick={() => pickPast(h)} disabled={own}>
                    {h.coverUrl ? <img src={h.coverUrl} alt="" loading="lazy" /> : <span className="noimg pp-noimg" />}
                    <span className="grow">
                      <span className="h-title">{h.title || "–"}</span>
                      <span className="muted small">
                        {[h.deviceName, h.endTime ? fmtDateTime(h.endTime) : "", secs ? fmtDuration(secs) : "",
                          g.planned ? (h.status === 3 ? `≈ ${fmtGrams(g.used)}` : fmtGrams(g.planned)) : ""].filter(Boolean).join(" · ")}
                      </span>
                    </span>
                    <span className="swatches">{parseRunFilaments(h.filaments).map((f, i) => <i key={i} style={{ background: f.color }} />)}</span>
                    <span className={`result-pill ${h.status === 2 ? "ok" : h.status === 3 ? "bad" : ""}`}>{h.status === 2 ? "Fertig" : h.status === 3 ? "Abgebrochen" : "?"}</span>
                    {own ? <span className="ok-badge small">zugeordnet</span> : h.orderId ? <span className="muted small">#{h.orderNumber}</span> : null}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </Modal>
  );
}
