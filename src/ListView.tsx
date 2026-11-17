import { useMemo, useState } from "react";
import type { OrderSummary, Status } from "./api";
import { ALL_STATUSES, STATUS, daysUntil, fmtDate } from "./status";
import type { ViewCtx } from "./App";

type Sort = "number" | "due" | "updated" | "status";
type Filter = "offen" | "alle" | "archiv" | Status;

const CLOSED: Status[] = ["uebergeben", "abgelehnt"];

export function ListView({ orders, open, openId }: ViewCtx) {
  const [filter, setFilter] = useState<Filter>("offen");
  const [sort, setSort] = useState<Sort>("number");

  const rows = useMemo(() => {
    const f = orders.filter((o) =>
      filter === "archiv" ? !!o.archivedAt
        : filter === "alle" ? true
          : o.archivedAt ? false
            : filter === "offen" ? !CLOSED.includes(o.status) : o.status === filter,
    );
    const by: Record<Sort, (a: OrderSummary, b: OrderSummary) => number> = {
      number: (a, b) => b.number - a.number,
      due: (a, b) => (a.dueDate ?? "9999").localeCompare(b.dueDate ?? "9999"),
      updated: (a, b) => b.updatedAt.localeCompare(a.updatedAt),
      status: (a, b) => ALL_STATUSES.indexOf(a.status) - ALL_STATUSES.indexOf(b.status) || a.position - b.position,
    };
    return [...f].sort(by[sort]);
  }, [orders, filter, sort]);

  return (
    <div className="list-view">
      <div className="filters">
        <label>
          Zeigen
          <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
            <option value="offen">Offene</option>
            <option value="alle">Alle (mit Archiv)</option>
            <option value="archiv">Archiv</option>
            {ALL_STATUSES.map((s) => (
              <option key={s} value={s}>{STATUS[s].label}</option>
            ))}
          </select>
        </label>
        <label>
          Sortieren
          <select value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
            <option value="number">Neueste zuerst</option>
            <option value="due">Fälligkeit</option>
            <option value="updated">Zuletzt geändert</option>
            <option value="status">Status</option>
          </select>
        </label>
        <span className="muted">{rows.length} Aufträge</span>
      </div>
      <table className="table">
        <thead>
          <tr>
            <th>Nr.</th><th>Titel</th><th>Kunde</th><th>Status</th><th>Fällig</th><th>Teile</th><th>Geändert</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((o) => {
            const d = daysUntil(o.dueDate);
            return (
              <tr key={o.id} className={o.id === openId ? "active" : ""} onClick={() => open(o.id)}>
                <td className="nr">#{o.number}</td>
                <td>
                  {o.title}
                  {o.priority > 0 && <span className={`prio p${o.priority}`}>{o.priority === 2 ? "dringend" : "hoch"}</span>}
                  {o.archivedAt && <span className="tag">Archiv</span>}
                </td>
                <td>{o.personName ?? <span className="muted">–</span>}</td>
                <td>
                  <span className="status-pill" style={{ ["--c" as string]: STATUS[o.status].color }}>
                    {STATUS[o.status].label}
                  </span>
                </td>
                <td className={!CLOSED.includes(o.status) && d !== null && d < 0 ? "late" : ""}>{fmtDate(o.dueDate)}</td>
                <td>{o.itemCount || ""}</td>
                <td className="muted">{fmtDate(o.updatedAt)}</td>
              </tr>
            );
          })}
          {rows.length === 0 && (
            <tr><td colSpan={7} className="empty">Keine Aufträge</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
