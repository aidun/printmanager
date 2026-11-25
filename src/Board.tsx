import { useState } from "react";
import { Archive, CalendarDays, ExternalLink, ListChecks, Paperclip, User } from "lucide-react";
import { api, fileSrc, orderFolder, type OrderSummary, type Status } from "./api";
import { BOARD_STATUSES, SIDE_STATUSES, STATUS, daysUntil, fmtDate } from "./status";
import type { ViewCtx } from "./App";

export function Board({ info, orders: all, reload, notify, fail, open, openId, openInStudio }: ViewCtx) {
  const [showSide, setShowSide] = useState(false);
  const orders = all.filter((o) => !o.archivedAt);
  const [dragOver, setDragOver] = useState<Status | null>(null);
  const columns = showSide ? [...BOARD_STATUSES, ...SIDE_STATUSES] : BOARD_STATUSES;
  const sideCount = orders.filter((o) => SIDE_STATUSES.includes(o.status)).length;

  async function drop(status: Status, e: React.DragEvent) {
    e.preventDefault();
    setDragOver(null);
    const id = e.dataTransfer.getData("text/x-order");
    if (!id) return;
    try {
      await api.setStatus(id, status);
      reload();
    } catch (err) {
      fail(err);
    }
  }

  return (
    <div className="board-wrap">
      <div className="board">
        {columns.map((s) => {
          const list = orders.filter((o) => o.status === s).sort((a, b) => a.position - b.position);
          return (
            <section
              key={s}
              className={`column ${dragOver === s ? "over" : ""}`}
              onDragOver={(e) => {
                if (e.dataTransfer.types.includes("text/x-order")) {
                  e.preventDefault();
                  setDragOver(s);
                }
              }}
              onDragLeave={() => setDragOver((d) => (d === s ? null : d))}
              onDrop={(e) => drop(s, e)}
            >
              <header title={STATUS[s].hint}>
                <span className="dot" style={{ background: STATUS[s].color }} />
                {STATUS[s].label}
                <span className="count">{list.length}</span>
                {s === "uebergeben" && list.length > 0 && (
                  <button
                    className="icon-btn"
                    aria-label="Archivieren"
                    title="Archivieren: Übergebene und abgelehnte Aufträge ins Archiv legen. Sie bleiben in Suche, Liste („Archiv“) und Statistik."
                    onClick={async () => {
                      try {
                        const n = await api.archiveClosed(0);
                        notify(n === 1 ? "1 Auftrag archiviert" : `${n} Aufträge archiviert`);
                        reload();
                      } catch (err) {
                        fail(err);
                      }
                    }}
                  >
                    <Archive size={14} />
                  </button>
                )}
              </header>
              <div className="cards">
                {list.map((o) => (
                  <Card key={o.id} o={o} dataDir={info?.dataDir} active={o.id === openId} onOpen={() => open(o.id)} onStudio={openInStudio} />
                ))}
                {list.length === 0 && <div className="empty">–</div>}
              </div>
            </section>
          );
        })}
      </div>
      <button className="link side-toggle" onClick={() => setShowSide((v) => !v)}>
        {showSide ? "Pausiert/Abgelehnt ausblenden" : `Pausiert/Abgelehnt anzeigen (${sideCount})`}
      </button>
    </div>
  );
}

function Card({
  o, dataDir, active, onOpen, onStudio,
}: { o: OrderSummary; dataDir?: string; active: boolean; onOpen: () => void; onStudio: (attachmentId: string) => void }) {
  const days = daysUntil(o.dueDate);
  const open = !["fertig", "uebergeben", "abgelehnt"].includes(o.status);
  return (
    <article
      className={`card prio-${o.priority} ${active ? "active" : ""}`}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("text/x-order", o.id);
        e.dataTransfer.effectAllowed = "move";
      }}
      onClick={onOpen}
      onKeyDown={(e) => e.key === "Enter" && onOpen()}
      tabIndex={0}
    >
      {o.coverPath && dataDir && (
        <img className="cover" src={fileSrc(orderFolder(dataDir, o.number), o.coverPath)} alt="" loading="lazy" />
      )}
      <div className="card-body">
        <div className="card-top">
          <span className="nr">#{o.number}</span>
          {o.priority > 0 && <span className={`prio p${o.priority}`}>{o.priority === 2 ? "dringend" : "hoch"}</span>}
        </div>
        <div className="title">{o.title}</div>
        <div className="meta">
          {o.personName && <span><User size={12} /> {o.personName}</span>}
          {o.dueDate && (
            <span className={open && days !== null && days < 0 ? "late" : open && days !== null && days <= 3 ? "soon" : ""}>
              <CalendarDays size={12} /> {fmtDate(o.dueDate)}
            </span>
          )}
          {o.itemCount > 0 && <span title="Teile"><ListChecks size={12} /> {o.itemCount}</span>}
          {o.attachmentCount > 0 && <span title="Anhänge"><Paperclip size={12} /> {o.attachmentCount}</span>}
        </div>
        {o.printFileId && !["fertig", "uebergeben", "abgelehnt"].includes(o.status) && (
          <button
            className="studio-btn"
            title="Druckdatei in Bambu Studio öffnen"
            onClick={(e) => {
              e.stopPropagation();
              onStudio(o.printFileId!);
            }}
          >
            <ExternalLink size={12} /> In Studio öffnen
          </button>
        )}
      </div>
    </article>
  );
}
