import { useEffect, useMemo, useState } from "react";
import {
  AlarmClock, CheckCircle2, ClipboardCheck, Inbox, Layers, PackageCheck, Printer, Wrench, type LucideIcon,
} from "lucide-react";
import { api, fmtGrams, spoolTitle, type OrderSummary, type PrinterView, type Spool, type Status } from "./api";
import { PrinterCard } from "./PrintersView";
import { SOON_DAYS, STATUS, count, daysUntil, fmtDate, isDue, isOpen } from "./status";
import { LOW } from "./FilamentView";
import type { ViewCtx } from "./App";



/** Startseite: was heute ansteht, ohne durch Board und Liste zu suchen */
export function TodayView({ orders, open, openId, fail, rev, onShowFilament }: ViewCtx & { rev: number; onShowFilament: () => void }) {
  const [spools, setSpools] = useState<Spool[]>([]);
  const [printers, setPrinters] = useState<PrinterView[]>([]);
  useEffect(() => {
    api.spools().then(setSpools).catch(fail);
    const loadPrinters = () => api.printers().then((r) => setPrinters(r.connected && r.enabled ? r.printers : [])).catch(() => {});
    loadPrinters();
    const t = window.setInterval(loadPrinters, 20000);
    return () => window.clearInterval(t);
  }, [fail, rev]);
  const busyPrinters = printers.filter((p) => p.state === "printing" || p.state === "paused" || p.state === "failed");

  const s = useMemo(() => {
    const open = orders.filter(isOpen);
    const active = open.filter((o) => o.status !== "pausiert");
    const by = (st: Status) => active.filter((o) => o.status === st).sort((a, b) => a.position - b.position);
    const due = active
      .filter(isDue)
      .sort((a, b) => (a.dueDate ?? "").localeCompare(b.dueDate ?? ""));
    const runs = orders.filter((o) => !o.archivedAt && (o.runsOpen > 0 || o.runsUndeducted > 0));
    const low = spools
      .filter((x) => !x.hidden && !x.missing && x.remainingGrams < LOW)
      .sort((a, b) => a.remainingGrams - b.remainingGrams);
    return {
      due, runs, low,
      inbox: [...by("eingang"), ...by("klaeren")],
      printing: by("druckt"),
      post: by("nacharbeit"),
      ready: by("fertig"),
      openCount: open.length,
    };
  }, [orders, spools]);

  const today = new Date().toLocaleDateString("de-DE", { weekday: "long", day: "numeric", month: "long" });
  const nothing = !s.due.length && !s.runs.length && !s.inbox.length && !s.printing.length && !s.post.length && !s.ready.length;
  const row = (o: OrderSummary, extra?: React.ReactNode) => <Row key={o.id} o={o} active={o.id === openId} onOpen={() => open(o.id)} extra={extra} />;

  return (
    <div className="today-view">
      <header className="toolbar">
        <h1>Heute</h1>
        <span className="muted">{today} · {count(s.openCount, "offener Auftrag", "offene Aufträge")}</span>
      </header>

      <div className="kpi-strip">
        <Kpi icon={AlarmClock} label="Fällig" value={s.due.length} alert={s.due.some((o) => (daysUntil(o.dueDate) ?? 1) < 0)} />
        <Kpi icon={Printer} label="Druckt gerade" value={s.printing.length} />
        <Kpi icon={ClipboardCheck} label="Drucke auswerten" value={s.runs.length} />
        <Kpi icon={PackageCheck} label="Zur Übergabe bereit" value={s.ready.length} />
      </div>

      {nothing && (
        <div className="empty-state">
          <CheckCircle2 size={28} />
          <b>Alles erledigt</b>
          <span>Neue Aufträge legst du mit ⌘N an oder ziehst Dateien ins Fenster.</span>
        </div>
      )}

      {busyPrinters.length > 0 && (
        <div className="printer-grid today-printers">
          {busyPrinters.map((p) => <PrinterCard key={p.id} p={p} open={open} />)}
        </div>
      )}

      <div className="today-grid">
        <Block icon={AlarmClock} title="Fällig" hint={`überfällig oder in den nächsten ${SOON_DAYS} Tagen`} tone="late" list={s.due} render={(o) => row(o)} />
        <Block
          icon={ClipboardCheck}
          title="Drucke auswerten"
          hint="Ergebnis eintragen oder Verbrauch abziehen"
          list={s.runs}
          render={(o) =>
            row(o, (
              <span>
                {[o.runsOpen > 0 && `${o.runsOpen} ohne Ergebnis`, o.runsUndeducted > 0 && `${o.runsUndeducted} nicht abgezogen`]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            ))
          }
        />
        <Block icon={Inbox} title="Neu & zu klären" hint="Eingang und Rückfragen" list={s.inbox} render={(o) => row(o)} />
        <Block icon={Printer} title="Druckt gerade" list={s.printing} render={(o) => row(o)} />
        <Block icon={Wrench} title="Nacharbeit" list={s.post} render={(o) => row(o)} />
        <Block icon={PackageCheck} title="Zur Übergabe bereit" list={s.ready} render={(o) => row(o)} />
        {s.low.length > 0 && (
          <section className="today-block">
            <h3><Layers size={14} /> Filament knapp <span className="count">{s.low.length}</span></h3>
            <ul className="today-list">
              {s.low.map((x) => (
                <li key={x.id} className="today-row" onClick={onShowFilament}>
                  <i className="sw" style={{ background: x.color || "transparent" }} />
                  <span className="grow">{spoolTitle(x)}</span>
                  <span className={x.remainingGrams <= 0 ? "late" : "warn"}>{fmtGrams(x.remainingGrams)}</span>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}

function Kpi({ icon: Icon, label, value, alert }: { icon: LucideIcon; label: string; value: number; alert?: boolean }) {
  return (
    <div className={`kpi-tile ${alert ? "alert" : ""}`}>
      <span className="kpi-label"><Icon size={13} /> {label}</span>
      <span className="kpi-num">{value}</span>
    </div>
  );
}

function Block({
  icon: Icon, title, hint, list, render, tone,
}: { icon: LucideIcon; title: string; hint?: string; list: OrderSummary[]; render: (o: OrderSummary) => React.ReactNode; tone?: "late" }) {
  if (!list.length) return null;
  return (
    <section className={`today-block ${tone ?? ""}`}>
      <h3 title={hint}>
        <Icon size={14} /> {title} <span className="count">{list.length}</span>
      </h3>
      <ul className="today-list">{list.map(render)}</ul>
    </section>
  );
}

function Row({ o, active, onOpen, extra }: { o: OrderSummary; active: boolean; onOpen: () => void; extra?: React.ReactNode }) {
  const d = daysUntil(o.dueDate);
  const dueText = d === null ? null : d < 0 ? `${-d} T überfällig` : d === 0 ? "heute fällig" : d === 1 ? "morgen" : `bis ${fmtDate(o.dueDate)}`;
  return (
    <li
      className={`today-row ${active ? "active" : ""}`}
      onClick={onOpen}
      onKeyDown={(e) => e.key === "Enter" && onOpen()}
      tabIndex={0}
    >
      <span className="dot" style={{ background: STATUS[o.status].color }} title={STATUS[o.status].label} />
      <span className="nr">#{o.number}</span>
      <span className="grow">
        <span className="row-title">{o.title}</span>
        <span className="row-meta">
          {o.personName && <span>{o.personName}</span>}
          {extra}
        </span>
      </span>
      {o.priority > 0 && <span className={`prio p${o.priority}`}>{o.priority === 2 ? "dringend" : "hoch"}</span>}
      {dueText && <span className={`due-badge ${d! < 0 ? "late" : d! <= 1 ? "soon" : ""}`}>{dueText}</span>}
    </li>
  );
}
