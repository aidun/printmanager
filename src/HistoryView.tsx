import { useCallback, useEffect, useMemo, useState } from "react";
import { History, Link2, RefreshCw, Search } from "lucide-react";
import {
  api, fmtDuration, fmtGrams, historyGrams, historySeconds, parseRunFilaments, USAGE_METHOD, type HistoryEntry, type OrderSummary,
} from "./api";
import { OrderPicker } from "./OrderPicker";
import { fmtDateTime } from "./status";

type Filter = "alle" | "zugeordnet" | "offen" | "fehler";
type Period = "7" | "30" | "90" | "alle";

const RESULT: Record<number, { label: string; cls: string }> = {
  2: { label: "Fertig", cls: "ok" },
  3: { label: "Abgebrochen", cls: "bad" },
};

const MATCHED: Record<string, string> = {
  manuell: "von Hand",
  nummer: "per Nummer",
  datei: "per Dateiname",
  titel: "per Titel",
};

/** Alle Drucke aus dem Druckverlauf des Bambu-Kontos, mit Zuordnung zu Aufträgen */
export function HistoryView({
  rev, orders, open, notify, fail, onSettings,
}: { rev: number; orders: OrderSummary[]; open: (id: string) => void; notify: (t: string) => void; fail: (e: unknown) => void; onSettings: () => void }) {
  const [list, setList] = useState<HistoryEntry[] | null>(null);
  const [connected, setConnected] = useState(true);
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Filter>("alle");
  const [device, setDevice] = useState("");
  const [period, setPeriod] = useState<Period>("alle");
  const [material, setMaterial] = useState("");
  const [busy, setBusy] = useState(false);
  const [assigning, setAssigning] = useState<string | null>(null);

  const load = useCallback(() => {
    api.history().then(setList).catch(fail);
    api.bambuStatus().then((b) => setConnected(b.connected)).catch(() => setConnected(false));
  }, [fail]);
  useEffect(() => {
    load();
  }, [load, rev]);

  const sync = async () => {
    setBusy(true);
    try {
      const r = await api.historySync();
      notify(`Druckverlauf abgeglichen: ${r.fetched} von ${r.total} Drucken${r.added ? `, ${r.added} neu` : ""}${r.matched ? `, ${r.matched} zugeordnet` : ""}`);
      load();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const assign = async (h: HistoryEntry, orderId: string | null) => {
    setAssigning(null);
    try {
      await api.historyAssign(h.id, orderId);
      notify(orderId ? "Druck zugeordnet – Dauer und Gramm stehen jetzt im Auftrag" : "Zuordnung aufgehoben");
      load();
    } catch (e) {
      fail(e);
    }
  };

  const devices = useMemo(() => [...new Set((list ?? []).map((h) => h.deviceName).filter(Boolean))].sort(), [list]);
  const materials = useMemo(
    () => [...new Set((list ?? []).flatMap((h) => parseRunFilaments(h.filaments).map((f) => f.type)).filter(Boolean))].sort(),
    [list],
  );
  const rows = useMemo(() => {
    const s = q.trim().toLowerCase();
    return (list ?? [])
      .filter((h) => !device || h.deviceName === device)
      .filter((h) => {
        if (period === "alle") return true;
        const t = h.endTime ?? h.startTime;
        return !!t && Date.now() - new Date(t).getTime() <= Number(period) * 86400000;
      })
      .filter((h) => !material || parseRunFilaments(h.filaments).some((f) => f.type === material))
      .filter((h) =>
        filter === "zugeordnet" ? !!h.orderId : filter === "offen" ? !h.orderId : filter === "fehler" ? h.status === 3 : true,
      )
      .filter((h) => !s || `${h.title} ${h.designTitle} ${h.deviceName} ${h.orderTitle ?? ""} ${h.personName ?? ""} ${h.orderNumber ?? ""}`.toLowerCase().includes(s));
  }, [list, q, filter, device, period, material]);

  const sum = useMemo(() => {
    const grams = rows.reduce((a, h) => a + historyGrams(h).used, 0);
    const secs = rows.reduce((a, h) => a + (historySeconds(h) ?? 0), 0);
    return { grams, secs };
  }, [rows]);

  return (
    <div className="history-view">
      <header className="toolbar">
        <h1>Druckverlauf</h1>
        <span className="page-sub">{list ? `${rows.length} Drucke · ${fmtDuration(sum.secs)} · ${fmtGrams(sum.grams)}` : "lädt …"}</span>
        <label className="search">
          <Search size={14} />
          <input type="search" placeholder="Titel, Modell, Auftrag, Kunde …" value={q} onChange={(e) => setQ(e.target.value)} />
        </label>
        <button className="primary" onClick={sync} disabled={busy || !connected}>
          <RefreshCw size={13} className={busy ? "spin" : ""} /> {busy ? "Gleicht ab …" : "Aktualisieren"}
        </button>
      </header>

      <p className="hint pad">
        Alle Drucke aus deinem Bambu-Konto. Die Dauer ist echt (Start bis Ende), die Gramm stammen aus dem Slicer – der Drucker
        misst den Verbrauch nicht. Drucke mit Auftragsnummer im Namen ordnet die App selbst zu; alles andere ordnest du hier zu.
        Dauer und Gramm kommen dann an den Druck im Auftrag.
      </p>

      <div className="filters pad">
        <div className="seg" role="group" aria-label="Filter">
          {([["alle", "Alle"], ["offen", "Ohne Auftrag"], ["zugeordnet", "Mit Auftrag"], ["fehler", "Abgebrochen"]] as const).map(([id, label]) => (
            <button key={id} className={filter === id ? "active" : ""} onClick={() => setFilter(id)}>{label}</button>
          ))}
        </div>
        <div className="seg" role="group" aria-label="Zeitraum">
          {([["7", "7 Tage"], ["30", "30 Tage"], ["90", "90 Tage"], ["alle", "Alles"]] as const).map(([id, label]) => (
            <button key={id} className={period === id ? "active" : ""} onClick={() => setPeriod(id)}>{label}</button>
          ))}
        </div>
        {materials.length > 1 && (
          <label>
            Material
            <select value={material} onChange={(e) => setMaterial(e.target.value)}>
              <option value="">alle</option>
              {materials.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </label>
        )}
        {devices.length > 1 && (
          <label>
            Drucker
            <select value={device} onChange={(e) => setDevice(e.target.value)}>
              <option value="">alle</option>
              {devices.map((d) => <option key={d} value={d}>{d}</option>)}
            </select>
          </label>
        )}
      </div>

      {!connected && (list?.length ?? 0) === 0 ? (
        <div className="empty-state">
          <History size={28} />
          <b>Kein Druckverlauf</b>
          <span>Der Druckverlauf kommt aus dem Bambu-Konto.</span>
          <button className="primary" onClick={onSettings}>Bambu-Konto verbinden</button>
        </div>
      ) : (
        <div className="pad">
          <table className="table history">
            <thead>
              <tr><th /><th>Druck</th><th>Drucker</th><th>Ende</th><th className="num">Dauer</th><th>Filament</th><th>Ergebnis</th><th>Auftrag</th></tr>
            </thead>
            <tbody>
              {rows.map((h) => {
                const fils = parseRunFilaments(h.filaments);
                const g = historyGrams(h);
                const estimated = h.status === 3 && (h.usedFactor ?? 1) < 1;
                const secs = historySeconds(h);
                const res = RESULT[h.status] ?? { label: "Läuft/unbekannt", cls: "" };
                return (
                  <tr key={h.id} className="plain-row">
                    <td className="thumb-cell">{h.coverUrl ? <img src={h.coverUrl} alt="" loading="lazy" /> : <span className="noimg" />}</td>
                    <td>
                      <div className="h-title" title={h.title}>{h.title || "–"}</div>
                      {h.designTitle && <div className="muted small ellipsis">{h.designTitle}</div>}
                    </td>
                    <td className="muted">{h.deviceName}</td>
                    <td className="muted nowrap">{h.endTime ? fmtDateTime(h.endTime) : h.startTime ? fmtDateTime(h.startTime) : ""}</td>
                    <td className="num nowrap" title={h.plannedSeconds ? `geplant: ${fmtDuration(h.plannedSeconds)}` : undefined}>{secs ? fmtDuration(secs) : ""}</td>
                    <td>
                      <span className="swatches">{fils.map((f, i) => <i key={i} style={{ background: f.color || "transparent" }} title={`${f.type} ${fmtGrams(f.grams)}`} />)}</span>{" "}
                      {estimated ? (
                        <span className="nowrap" title={`${USAGE_METHOD[h.usedMethod ?? ""] ?? "geschätzt"}${h.progressAtEnd != null ? ` (${h.progressAtEnd} %)` : ""}`}>
                          ≈ {fmtGrams(g.used)} <span className="muted small">von {fmtGrams(g.planned)}</span>
                        </span>
                      ) : (
                        <span className="nowrap">{g.planned > 0 ? fmtGrams(g.planned) : ""}</span>
                      )}
                    </td>
                    <td><span className={`result-pill ${res.cls}`}>{res.label}</span></td>
                    <td className="assign-cell">
                      {assigning === h.id ? (
                        <OrderPicker orders={orders} value={h.orderId} allowNone onPick={(id) => assign(h, id)} onCancel={() => setAssigning(null)} />
                      ) : h.orderId ? (
                        <span className="assigned">
                          <button className="link" onClick={() => open(h.orderId!)}><Link2 size={12} /> #{h.orderNumber} {h.orderTitle}</button>
                          <span className="muted small">{h.personName ? `${h.personName} · ` : ""}{h.matchedBy ? MATCHED[h.matchedBy] ?? "" : ""}</span>
                          <button className="ghost small-btn" onClick={() => setAssigning(h.id)}>Ändern</button>
                        </span>
                      ) : (
                        <button className="small-btn" onClick={() => setAssigning(h.id)}>Zuordnen …</button>
                      )}
                    </td>
                  </tr>
                );
              })}
              {list && rows.length === 0 && (
                <tr><td colSpan={8} className="empty">{list.length ? "Keine Drucke für diesen Filter." : "Noch kein Druckverlauf. Klicke auf „Aktualisieren“."}</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
