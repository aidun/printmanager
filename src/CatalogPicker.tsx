import { useEffect, useMemo, useState } from "react";
import { Box, Check } from "lucide-react";
import { api, catalogSrc, parseFilaments, type CatalogEntry } from "./api";

export interface CatalogPick {
  entry: CatalogEntry;
  quantity: number;
  colorWish: string;
}

interface Props {
  dataDir: string;
  picks: CatalogPick[];
  onChange: (picks: CatalogPick[]) => void;
  fail: (e: unknown) => void;
}

/** Projekte aus dem Katalog auswählen, mit Anzahl und Farbwunsch */
export function CatalogPicker({ dataDir, picks, onChange, fail }: Props) {
  const [all, setAll] = useState<CatalogEntry[]>([]);
  const [q, setQ] = useState("");

  useEffect(() => {
    api.catalog(false).then(setAll).catch(fail);
  }, [fail]);

  const list = useMemo(() => {
    const s = q.trim().toLowerCase();
    return s ? all.filter((e) => `${e.title} ${e.tags} ${e.designer}`.toLowerCase().includes(s)) : all;
  }, [all, q]);

  const picked = (id: string) => picks.find((p) => p.entry.id === id);
  const toggle = (e: CatalogEntry) =>
    onChange(picked(e.id) ? picks.filter((p) => p.entry.id !== e.id) : [...picks, { entry: e, quantity: 1, colorWish: "" }]);
  const patch = (id: string, p: Partial<CatalogPick>) => onChange(picks.map((x) => (x.entry.id === id ? { ...x, ...p } : x)));

  if (!all.length) {
    return <p className="hint">Der Katalog ist noch leer. Projekte fügst du unter „Katalog“ hinzu.</p>;
  }

  return (
    <div className="catalog-picker">
      <input type="search" placeholder="Katalog durchsuchen …" value={q} onChange={(e) => setQ(e.target.value)} />
      <div className="pick-grid">
        {list.map((e) => {
          const p = picked(e.id);
          const src = catalogSrc(dataDir, e);
          return (
            <div key={e.id} className={`pick ${p ? "on" : ""}`}>
              <button className="pick-main" onClick={() => toggle(e)} aria-pressed={!!p}>
                {src ? <img src={src} alt="" /> : <div className="noimg"><Box size={24} strokeWidth={1.4} /></div>}
                <span className="pick-title">{e.title}</span>
                <span className="swatches">
                  {parseFilaments(e.filaments).map((f, i) => (
                    <i key={i} style={{ background: f.color || "transparent" }} title={f.type} />
                  ))}
                </span>
                {p && <span className="tick" aria-hidden><Check size={13} strokeWidth={3} /></span>}
              </button>
              {p && (
                <div className="pick-opts">
                  <input type="number" min={1} className="qty" value={p.quantity} aria-label="Anzahl"
                    onChange={(ev) => patch(e.id, { quantity: Math.max(1, Number(ev.target.value)) })} />
                  <input className="grow" placeholder="Farbwunsch" value={p.colorWish}
                    onChange={(ev) => patch(e.id, { colorWish: ev.target.value })} />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
