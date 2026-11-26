import { useEffect, useMemo, useState } from "react";
import { api, fmtDuration, fmtGrams, historySeconds, parseRunFilaments, type OrderSummary, type Stats, type StatRun } from "./api";

type Range = "12m" | "jahr" | "alles";

const RANGES: { id: Range; label: string }[] = [
  { id: "12m", label: "Letzte 12 Monate" },
  { id: "jahr", label: "Dieses Jahr" },
  { id: "alles", label: "Alles" },
];

const monthKey = (iso: string) => {
  const d = new Date(iso);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
};
const monthLabel = (key: string, long = false) =>
  new Date(`${key}-01T00:00:00`).toLocaleDateString("de-DE", long ? { month: "long", year: "numeric" } : { month: "short" });

function start(range: Range): Date | null {
  const now = new Date();
  if (range === "jahr") return new Date(now.getFullYear(), 0, 1);
  if (range === "12m") return new Date(now.getFullYear(), now.getMonth() - 11, 1);
  return null;
}

/** Monate von `from` bis heute als „JJJJ-MM“ */
function months(from: Date) {
  const out: string[] = [];
  const d = new Date(from.getFullYear(), from.getMonth(), 1);
  const end = new Date();
  while (d <= end) {
    out.push(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`);
    d.setMonth(d.getMonth() + 1);
  }
  return out;
}

interface Agg { orders: number; runs: number; grams: number; seconds: number }
const empty = (): Agg => ({ orders: 0, runs: 0, grams: 0, seconds: 0 });

export function StatsView({ orders, rev, fail }: { orders: OrderSummary[]; rev: number; fail: (e: unknown) => void }) {
  const [data, setData] = useState<Stats | null>(null);
  const [range, setRange] = useState<Range>("12m");
  /** Quelle: alle Drucke aus dem Bambu-Konto oder nur die Drucke in Aufträgen */
  const [source, setSource] = useState<"konto" | "auftraege" | null>(null);
  /** fertig = erfolgreich + abgebrochen; geplant = gesliced, noch nicht gedruckt */
  const [kind, setKind] = useState<"fertig" | "ok" | "fehldruck" | "geplant">("fertig");
  const hasHistory = (data?.history.length ?? 0) > 0;
  const src = source ?? (hasHistory ? "konto" : "auftraege");

  useEffect(() => {
    api.stats().then(setData).catch(fail);
  }, [fail, rev]);

  const r = useMemo(() => {
    if (!data) return null;
    const from = start(range);
    const inRange = (iso: string) => !from || new Date(iso) >= from;
    // Drucke aus dem Konto haben echte Dauer; Kunde nur, wenn einem Auftrag zugeordnet
    const base: StatRun[] = src === "konto"
      ? data.history.map((h) => ({
          createdAt: h.endTime ?? h.startTime ?? "",
          result: h.status === 3 ? "fehldruck" : h.status === 2 ? "ok" : "offen",
          printSeconds: historySeconds(h),
          // abgebrochen: geschätzter tatsächlicher Verbrauch
          filaments: JSON.stringify(parseRunFilaments(h.filaments).map((f) => ({ ...f, grams: f.grams * (h.usedFactor ?? 1) }))),
          orderNumber: h.orderNumber ?? 0,
          personName: h.orderId ? h.personName : "Ohne Auftrag",
        }))
      : data.runs;
    const planned = data.runs.filter((x) => x.result === "offen");
    const pool = kind === "geplant" ? planned : base.filter((x) => (kind === "fertig" ? x.result !== "offen" : x.result === kind));
    const runs = pool.filter((x) => x.createdAt && inRange(x.createdAt)).map((x) => ({ ...x, fils: parseRunFilaments(x.filaments) }));
    const handovers = data.handovers.filter((x) => inRange(x.createdAt));
    const created = orders.filter((o) => inRange(o.createdAt)).length;

    const gramsOf = (x: (typeof runs)[number]) => x.fils.reduce((s, f) => s + f.grams, 0);
    const total = runs.reduce((s, x) => s + gramsOf(x), 0);
    const seconds = runs.reduce((s, x) => s + (x.printSeconds ?? 0), 0);
    const failed = runs.filter((x) => x.result === "fehldruck");
    const failedGrams = failed.reduce((s, x) => s + gramsOf(x), 0);
    const rated = runs.filter((x) => x.result !== "offen").length;

    // Verbrauch pro Monat; ohne Zeitraum ab dem ersten Druck
    const first = from ?? (runs[0] ? new Date(runs[0].createdAt) : new Date());
    const perMonth = new Map(months(first).map((m) => [m, { grams: 0, runs: 0, seconds: 0 }]));
    for (const x of runs) {
      const m = perMonth.get(monthKey(x.createdAt));
      if (m) {
        m.grams += gramsOf(x);
        m.runs += 1;
        m.seconds += x.printSeconds ?? 0;
      }
    }

    const people = new Map<string, Agg>();
    const person = (n: string | null) => {
      const k = n || "Ohne Kunde";
      if (!people.has(k)) people.set(k, empty());
      return people.get(k)!;
    };
    for (const h of handovers) person(h.personName).orders += 1;
    for (const x of runs) {
      const p = person(x.personName);
      p.runs += 1;
      p.grams += gramsOf(x);
      p.seconds += x.printSeconds ?? 0;
    }

    const materials = new Map<string, Agg>();
    for (const x of runs) {
      for (const f of x.fils) {
        const k = f.type || "Unbekannt";
        const m = materials.get(k) ?? empty();
        m.grams += f.grams;
        m.runs += 1;
        materials.set(k, m);
      }
    }

    return {
      runs: runs.length, total, seconds, failed: failed.length, failedGrams, rated,
      handovers: handovers.length, created,
      perMonth: [...perMonth.entries()],
      people: [...people.entries()].sort((a, b) => b[1].grams - a[1].grams || b[1].orders - a[1].orders),
      materials: [...materials.entries()].sort((a, b) => b[1].grams - a[1].grams),
    };
  }, [data, orders, range, src, kind]);

  return (
    <div className="stats-view">
      <header className="toolbar">
        <h1>Statistik</h1>
        <div className="seg" role="group" aria-label="Quelle" title="Alle Drucke aus dem Bambu-Konto oder nur Drucke, die zu Aufträgen gehören">
          <button className={src === "konto" ? "active" : ""} onClick={() => setSource("konto")} disabled={!hasHistory}>Alle Drucke (Konto)</button>
          <button className={src === "auftraege" ? "active" : ""} onClick={() => setSource("auftraege")}>Nur Aufträge</button>
        </div>
        <div className="seg" role="group" aria-label="Drucke">
          {([["fertig", "Fertig"], ["ok", "Erfolgreich"], ["fehldruck", "Abgebrochen"], ["geplant", "Geplant"]] as const).map(([id, label]) => (
            <button key={id} className={kind === id ? "active" : ""} onClick={() => setKind(id)}>{label}</button>
          ))}
        </div>
        <div className="seg" role="group" aria-label="Zeitraum">
          {RANGES.map((x) => (
            <button key={x.id} className={range === x.id ? "active" : ""} onClick={() => setRange(x.id)}>{x.label}</button>
          ))}
        </div>
      </header>

      {r && (
        <div className="pad stats-body">
          <div className="kpis">
            <Kpi label="Filament verbraucht" value={fmtKg(r.total)} sub={r.failedGrams > 0 ? `davon ${fmtGrams(r.failedGrams)} Fehldruck` : undefined} />
            <Kpi label="Druckzeit" value={r.seconds ? fmtDuration(r.seconds) : "–"} sub={`${r.runs} Drucke`} />
            <Kpi
              label="Fehldrucke"
              value={String(r.failed)}
              sub={r.rated ? `${Math.round((r.failed / r.rated) * 100)} % der bewerteten Drucke` : "noch nichts bewertet"}
            />
            <Kpi label="Übergeben" value={String(r.handovers)} sub={`${r.created} Aufträge neu angelegt`} />
          </div>

          <section className="section">
            <h3>Filament pro Monat</h3>
            {r.runs === 0 ? (
              <p className="hint">
                Noch keine Drucke in diesem Zeitraum.
              </p>
            ) : (
              <MonthBars data={r.perMonth} />
            )}
          </section>

          <div className="stats-cols">
            <section className="section">
              <h3>Nach Kunde</h3>
              <table className="table plain">
                <thead>
                  <tr><th>Kunde</th><th className="num">Übergeben</th><th className="num">Drucke</th><th className="num">Filament</th><th className="num">Druckzeit</th></tr>
                </thead>
                <tbody>
                  {r.people.map(([name, a]) => (
                    <tr key={name}>
                      <td>{name}</td>
                      <td className="num">{a.orders || ""}</td>
                      <td className="num">{a.runs || ""}</td>
                      <td className="num">{a.grams ? fmtGrams(a.grams) : ""}</td>
                      <td className="num">{a.seconds ? fmtDuration(a.seconds) : ""}</td>
                    </tr>
                  ))}
                  {r.people.length === 0 && <tr><td colSpan={5} className="empty">Keine Daten</td></tr>}
                </tbody>
              </table>
            </section>

            <section className="section">
              <h3>Nach Material</h3>
              <table className="table plain">
                <thead>
                  <tr><th>Material</th><th className="num">Filament</th><th>Anteil</th></tr>
                </thead>
                <tbody>
                  {r.materials.map(([m, a]) => {
                    const pct = r.total ? (a.grams / r.total) * 100 : 0;
                    return (
                      <tr key={m}>
                        <td>{m}</td>
                        <td className="num">{fmtGrams(a.grams)}</td>
                        <td className="share">
                          <span className="share-bar"><span style={{ width: `${pct}%` }} /></span>
                          <span className="muted small">{Math.round(pct)} %</span>
                        </td>
                      </tr>
                    );
                  })}
                  {r.materials.length === 0 && <tr><td colSpan={3} className="empty">Keine Daten</td></tr>}
                </tbody>
              </table>
            </section>
          </div>
        </div>
      )}
    </div>
  );
}

function fmtKg(g: number) {
  return g >= 1000 ? `${(g / 1000).toLocaleString("de-DE", { maximumFractionDigits: 2 })} kg` : fmtGrams(g);
}

function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="kpi">
      <div className="muted small">{label}</div>
      <div className="kpi-value">{value}</div>
      {sub && <div className="muted small">{sub}</div>}
    </div>
  );
}

/** Säulen: Gramm pro Monat. Hover zeigt Gramm, Drucke und Druckzeit. */
function MonthBars({ data }: { data: [string, { grams: number; runs: number; seconds: number }][] }) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(...data.map(([, v]) => v.grams), 1);
  const h = hover !== null ? data[hover] : null;
  return (
    <div className="month-chart">
      <div className="month-tip" aria-live="polite">
        {h ? (
          <>
            <b>{monthLabel(h[0], true)}</b> · {fmtGrams(h[1].grams)} · {h[1].runs} Drucke
            {h[1].seconds > 0 && ` · ${fmtDuration(h[1].seconds)}`}
          </>
        ) : (
          <span className="muted">Höchster Monat: {fmtGrams(max)}</span>
        )}
      </div>
      <div className="month-bars" onMouseLeave={() => setHover(null)}>
        {data.map(([m, v], i) => (
          <div
            key={m}
            className={`month ${hover === i ? "hover" : ""}`}
            onMouseEnter={() => setHover(i)}
            title={`${monthLabel(m, true)}: ${fmtGrams(v.grams)}, ${v.runs} Drucke`}
          >
            <div className="month-track">
              {v.grams > 0 && <span style={{ height: `${(v.grams / max) * 100}%` }} />}
            </div>
            <span className="month-label">{monthLabel(m)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
