import { useCallback, useEffect, useState } from "react";
import { Link2, Printer as PrinterIcon, RefreshCw } from "lucide-react";
import { api, fmtDuration, fmtProgress, PRINTER_STATE, type OrderSummary, type PrintersResult, type PrinterView } from "./api";
import { fmtDateTime } from "./status";
import { OrderPicker } from "./OrderPicker";

/** Drucker aus dem Bambu-Konto. Nur Anzeige: Drucke startest du in Bambu Studio oder Bambu Handy. */
export function PrintersView({ rev, open, fail, onSettings, orders }: { rev: number; open: (id: string) => void; fail: (e: unknown) => void; onSettings: () => void; orders: OrderSummary[] }) {
  const [r, setR] = useState<PrintersResult | null>(null);
  const [busy, setBusy] = useState(false);
  const load = useCallback(() => api.printers().then(setR).catch(fail), [fail]);

  useEffect(() => {
    load();
    const t = window.setInterval(load, 10000);
    return () => window.clearInterval(t);
  }, [load, rev]);

  const refresh = async () => {
    setBusy(true);
    try {
      setR(await api.printersRefresh());
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="printers-view">
      <header className="toolbar">
        <h1>Drucker</h1>
        {r?.updatedAt && <span className="muted small">aktualisiert {fmtDateTime(r.updatedAt)}</span>}
        <button className="primary push" onClick={refresh} disabled={busy || !r?.connected}><RefreshCw size={13} className={busy ? "spin" : ""} /> {busy ? "Lädt …" : "Aktualisieren"}</button>
      </header>
      <p className="hint pad">
        Drucker aus deinem Bambu-Konto, jede Minute aktualisiert, während eines Drucks alle 20 Sekunden. Die App zeigt nur an.
        Drucke startest du weiter in Bambu Studio oder Bambu Handy. Passt der Druckauftrag zu einer Datei in einem Auftrag,
        steht der Auftrag dabei.
      </p>
      {r && !r.connected && (
        <div className="empty pad">
          Nicht mit dem Bambu-Konto verbunden. <button className="link" onClick={onSettings}>Zu den Einstellungen</button>
        </div>
      )}
      {r && r.connected && !r.enabled && (
        <div className="empty pad">
          Druckerstatus ist ausgeschaltet. <button className="link" onClick={onSettings}>In den Einstellungen einschalten</button>
        </div>
      )}
      {r && r.connected && r.enabled && r.printers.length === 0 && (
        <div className="empty pad">
          {r.updatedAt ? "Keine Drucker mit deinem Konto verbunden." : "Drucker werden geladen …"} Drucker verbindest du in Bambu Handy
          mit deinem Konto.
        </div>
      )}
      {r?.error && r.connected && <p className="warn pad">{r.error}</p>}
      <div className="printer-grid">
        {r?.printers.map((p) => (
          <PrinterCard key={p.id} p={p} open={open} orders={orders}
            onAssign={(orderId) => api.printerAssign(p.taskName!, orderId).then(setR).catch(fail)} />
        ))}
      </div>
    </div>
  );
}

const MATCHED: Record<string, string> = {
  manuell: "von Hand zugeordnet",
  nummer: "über die Auftragsnummer im Dateinamen",
  datei: "über den Dateinamen",
  titel: "über den Auftragstitel",
};

export function PrinterCard({
  p, open, orders, onAssign,
}: { p: PrinterView; open: (id: string) => void; orders?: OrderSummary[]; onAssign?: (orderId: string | null) => void }) {
  const active = p.state === "printing" || p.state === "paused";
  const [choosing, setChoosing] = useState(false);
  return (
    <article className={`printer st-${p.state}`}>
      <div className="printer-head">
        <PrinterIcon size={15} />
        <span className={`online-dot ${p.online ? "on" : ""}`} title={p.online ? "online" : "offline"} />
        <b className="grow">{p.name}</b>
        <span className="muted small">{p.model}</span>
        <span className={`printer-state st-${p.state}`}>{PRINTER_STATE[p.state]}</span>
      </div>
      {p.taskName && (active || p.state === "finished" || p.state === "failed") && (
        <div className="printer-task" title={p.taskName}>{p.taskName}</div>
      )}
      {active && (
        <>
          <div className="bar big" title={p.progress === null ? "Fortschritt unbekannt" : p.progressEstimated ? "Geschätzt aus Startzeit und geplanter Druckdauer" : `${p.progress} %`}>
            <span style={{ width: `${p.progress ?? 0}%`, background: "var(--s-druckt)" }} />
          </div>
          <div className="printer-meta muted small">
            {p.progress !== null && <span>{fmtProgress(p)}</span>}
            {p.remainingSeconds ? <span>noch {fmtDuration(p.remainingSeconds)}</span> : null}
          </div>
        </>
      )}
      {p.taskName && (active || p.state === "finished" || p.state === "failed") && (
        <div className="printer-order">
          {p.orderId ? (
            <button className="link order-link" onClick={() => open(p.orderId!)} title={p.matchedBy ? `Zugeordnet ${MATCHED[p.matchedBy]}` : undefined}>
              <Link2 size={12} /> #{p.orderNumber} {p.orderTitle}
            </button>
          ) : (
            <span className="muted small">Keinem Auftrag zugeordnet</span>
          )}
          {p.matchedBy && <span className="muted small">{p.matchedBy === "manuell" ? "· von Hand" : p.matchedBy === "nummer" ? "· per Nummer" : ""}</span>}
          {onAssign && !choosing && (
            <button className="ghost small-btn push" onClick={() => setChoosing(true)}>{p.orderId ? "Ändern" : "Zuordnen …"}</button>
          )}
        </div>
      )}
      {onAssign && choosing && (
        <OrderPicker orders={orders ?? []} value={p.matchedBy === "manuell" ? p.orderId : null} allowNone
          onPick={(id) => { onAssign(id); setChoosing(false); }} onCancel={() => setChoosing(false)} />
      )}
    </article>
  );
}
