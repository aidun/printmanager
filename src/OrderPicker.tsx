import { useMemo, useState } from "react";
import { Search, X } from "lucide-react";
import type { OrderSummary } from "./api";
import { STATUS } from "./status";

const CLOSED = ["uebergeben", "abgelehnt"];

/**
 * Auftrag suchen und wählen: Suche nach Nummer, Titel und Kunde, wahlweise nur offene Aufträge.
 * `onPick(null)` hebt eine Zuordnung auf (nur angeboten, wenn `allowNone`).
 */
export function OrderPicker({
  orders, value, onPick, onCancel, allowNone,
}: { orders: OrderSummary[]; value?: string | null; onPick: (id: string | null) => void; onCancel: () => void; allowNone?: boolean }) {
  const [q, setQ] = useState("");
  const [onlyOpen, setOnlyOpen] = useState(true);
  const [hi, setHi] = useState(0);

  const list = useMemo(() => {
    const s = q.trim().toLowerCase().replace(/^#/, "");
    return [...orders]
      .filter((o) => !onlyOpen || (!o.archivedAt && !CLOSED.includes(o.status)))
      .filter((o) => !s || `${o.number} ${o.title} ${o.personName ?? ""}`.toLowerCase().includes(s))
      .sort((a, b) => b.number - a.number)
      .slice(0, 30);
  }, [orders, q, onlyOpen]);

  return (
    <div className="order-picker" onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), onCancel())}>
      <div className="op-head">
        <label className="search">
          <Search size={13} />
          <input
            autoFocus
            value={q}
            placeholder="Nr., Titel oder Kunde …"
            onChange={(e) => { setQ(e.target.value); setHi(0); }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") { e.preventDefault(); setHi((h) => Math.min(h + 1, list.length - 1)); }
              else if (e.key === "ArrowUp") { e.preventDefault(); setHi((h) => Math.max(h - 1, 0)); }
              else if (e.key === "Enter" && list[hi]) { e.preventDefault(); onPick(list[hi].id); }
            }}
            aria-label="Auftrag suchen"
          />
        </label>
        <label className="check small"><input type="checkbox" checked={onlyOpen} onChange={(e) => setOnlyOpen(e.target.checked)} /> nur offene</label>
        <button className="icon-btn" onClick={onCancel} aria-label="Abbrechen" title="Abbrechen (Esc)"><X size={14} /></button>
      </div>
      <ul className="op-list" role="listbox">
        {allowNone && value && (
          <li><button className="op-opt none" onClick={() => onPick(null)}>Zuordnung aufheben</button></li>
        )}
        {list.map((o, i) => (
          <li key={o.id}>
            <button className={`op-opt ${i === hi ? "hi" : ""} ${o.id === value ? "on" : ""}`} onMouseEnter={() => setHi(i)} onClick={() => onPick(o.id)}>
              <span className="dot" style={{ background: STATUS[o.status].color }} title={STATUS[o.status].label} />
              <span className="nr">#{o.number}</span>
              <span className="grow ellipsis">{o.title}</span>
              <span className="muted small ellipsis">{o.personName ?? ""}</span>
            </button>
          </li>
        ))}
        {list.length === 0 && <li className="muted small op-empty">Kein Auftrag gefunden{onlyOpen ? " – „nur offene“ abwählen?" : "."}</li>}
      </ul>
    </div>
  );
}
