import { useEffect, useMemo, useState } from "react";
import { Box, Check } from "lucide-react";
import { api, catalogSrc, parseFilaments, type CatalogEntry, type MwCollection } from "./api";

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

const CATEGORY_KEY = "pm.pickerCategory";
const loadCategory = () => {
  try {
    return localStorage.getItem(CATEGORY_KEY) ?? "";
  } catch {
    return "";
  }
};
const saveCategory = (v: string) => {
  try {
    localStorage.setItem(CATEGORY_KEY, v);
  } catch {
    /* ohne Speicher: Auswahl gilt nur bis zum Schließen */
  }
};

/** Kategorien eines Eintrags: Namen seiner MakerWorld-Sammlungen und seine Stichworte, klein geschrieben */
function categoriesOf(e: CatalogEntry, collTitle: Map<string, string>): string[] {
  const names = [
    ...e.collectionIds.map((id) => collTitle.get(id)).filter((t): t is string => !!t),
    ...e.tags.split(/[,;]/),
  ];
  return [...new Set(names.map((n) => n.trim().toLowerCase()).filter(Boolean))];
}

/** Projekte aus dem Katalog auswählen, mit Anzahl und Farbwunsch */
export function CatalogPicker({ dataDir, picks, onChange, fail }: Props) {
  const [all, setAll] = useState<CatalogEntry[]>([]);
  const [q, setQ] = useState("");
  const [colls, setColls] = useState<MwCollection[]>([]);
  /** Kategorie, klein geschrieben; leer = alle */
  const [cat, setCat] = useState(loadCategory);

  useEffect(() => {
    api.catalog(false).then(setAll).catch(fail);
    api.mwCollections().then((l) => setColls(l.filter((c) => c.enabled))).catch(() => setColls([]));
  }, [fail]);

  const catsOf = useMemo(() => {
    const titles = new Map(colls.map((c) => [c.id, c.title]));
    return new Map(all.map((e) => [e.id, categoriesOf(e, titles)]));
  }, [all, colls]);

  // Kategorien mit Anzahl, gleiche Namen (zwei Sammlungen „Figuren“, Sammlung und Stichwort) zusammengefasst
  const categories = useMemo(() => {
    const label = new Map<string, string>();
    for (const c of colls) label.set(c.title.trim().toLowerCase(), c.title.trim());
    for (const e of all) for (const t of e.tags.split(/[,;]/)) if (t.trim() && !label.has(t.trim().toLowerCase())) label.set(t.trim().toLowerCase(), t.trim());
    const count = new Map<string, number>();
    for (const l of catsOf.values()) for (const k of l) count.set(k, (count.get(k) ?? 0) + 1);
    return [...count.entries()]
      .map(([key, n]) => ({ key, label: label.get(key) ?? key, n }))
      .sort((a, b) => a.label.localeCompare(b.label, "de"));
  }, [all, colls, catsOf]);

  // gemerkte Kategorie gibt es nicht mehr: alle zeigen
  const active = categories.some((c) => c.key === cat) ? cat : "";
  const chooseCat = (v: string) => {
    setCat(v);
    saveCategory(v);
  };

  const list = useMemo(() => {
    const s = q.trim().toLowerCase();
    return all
      .filter((e) => !active || (catsOf.get(e.id) ?? []).includes(active))
      .filter((e) => !s || `${e.title} ${e.tags} ${e.designer}`.toLowerCase().includes(s));
  }, [all, q, active, catsOf]);

  const picked = (id: string) => picks.find((p) => p.entry.id === id);
  const toggle = (e: CatalogEntry) =>
    onChange(picked(e.id) ? picks.filter((p) => p.entry.id !== e.id) : [...picks, { entry: e, quantity: 1, colorWish: "" }]);
  const patch = (id: string, p: Partial<CatalogPick>) => onChange(picks.map((x) => (x.entry.id === id ? { ...x, ...p } : x)));

  if (!all.length) {
    return <p className="hint">Der Katalog ist noch leer. Projekte fügst du unter „Katalog“ hinzu.</p>;
  }

  return (
    <div className="catalog-picker">
      <div className="pick-filter">
        <input type="search" className="grow" placeholder="Katalog durchsuchen …" value={q} onChange={(e) => setQ(e.target.value)} />
        {categories.length > 0 && (
          <select value={active} onChange={(e) => chooseCat(e.target.value)} aria-label="Kategorie">
            <option value="">Alle Kategorien ({all.length})</option>
            {categories.map((c) => <option key={c.key} value={c.key}>{c.label} ({c.n})</option>)}
          </select>
        )}
      </div>
      {list.length === 0 && (
        <p className="hint">
          Nichts gefunden.{" "}
          {active && <button className="link" onClick={() => chooseCat("")}>Alle Kategorien zeigen</button>}
        </p>
      )}
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
