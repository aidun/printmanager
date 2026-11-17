import { useEffect, useMemo, useRef, useState } from "react";
import { Check, Plus, Search, UserRound, X } from "lucide-react";
import { api, customerNo, EMPTY_PERSON, type Person } from "./api";

interface Props {
  people: Person[];
  /** gewählter Kunde (ID) */
  value: string | null;
  onChange: (id: string, created?: Person) => void;
  fail: (e: unknown) => void;
  autoFocus?: boolean;
  /** kompakt: zeigt den gewählten Kunden mit „Ändern“, Suche erst nach Klick */
  compact?: boolean;
}

/**
 * Kunde suchen und wählen. Gibt es keinen passenden Kunden, lässt er sich direkt hier anlegen
 * (Name, E-Mail, Telefon) und ist danach gewählt.
 */
export function CustomerPicker({ people, value, onChange, fail, autoFocus, compact }: Props) {
  const selected = people.find((p) => p.id === value) ?? null;
  const [open, setOpen] = useState(!compact || !selected);
  const [q, setQ] = useState("");
  const [hi, setHi] = useState(0);
  const [form, setForm] = useState<{ name: string; email: string; phone: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open && (autoFocus || compact)) inputRef.current?.focus();
  }, [open, autoFocus, compact]);

  const s = q.trim().toLowerCase();
  const matches = useMemo(() => {
    const list = s
      ? people.filter((p) => `${p.name} ${customerNo(p.number)} ${p.email} ${p.phone}`.toLowerCase().includes(s))
      : [...people].sort((a, b) => (b.lastOrderAt ?? "").localeCompare(a.lastOrderAt ?? "") || a.name.localeCompare(b.name));
    return list.slice(0, 8);
  }, [people, s]);
  const exact = people.some((p) => p.name.trim().toLowerCase() === s);
  const offerNew = s.length > 0 && !exact;
  const options = matches.length + (offerNew ? 1 : 0);

  const pick = (p: Person) => {
    onChange(p.id);
    setQ("");
    setForm(null);
    if (compact) setOpen(false);
  };

  const startNew = () => setForm({ name: q.trim(), email: "", phone: "" });

  async function create() {
    if (!form || !form.name.trim()) return;
    setBusy(true);
    try {
      const id = await api.savePerson({ ...EMPTY_PERSON, name: form.name.trim(), email: form.email, phone: form.phone });
      const created = (await api.people()).find((p) => p.id === id);
      onChange(id, created);
      setQ("");
      setForm(null);
      if (compact) setOpen(false);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  if (!open && selected) {
    return (
      <div className="cust-selected">
        <UserRound size={14} />
        <span className="grow"><b>{selected.name}</b> <span className="muted small">{customerNo(selected.number)}</span></span>
        <button className="small-btn" onClick={() => setOpen(true)}>Ändern</button>
      </div>
    );
  }

  return (
    <div className="cust-picker">
      {selected && !compact && (
        <div className="cust-selected on">
          <Check size={14} />
          <span className="grow">
            <b>{selected.name}</b> <span className="muted small">{customerNo(selected.number)}</span>
            {(selected.email || selected.phone) && <span className="muted small"> · {[selected.email, selected.phone].filter(Boolean).join(" · ")}</span>}
          </span>
        </div>
      )}
      {!form && (
        <>
          <label className="search cust-search">
            <Search size={14} />
            <input
              ref={inputRef}
              autoComplete="off"
              value={q}
              placeholder={selected ? "Anderen Kunden suchen …" : "Kunde suchen: Name, Nummer, E-Mail, Telefon …"}
              onChange={(e) => { setQ(e.target.value); setHi(0); }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") { e.preventDefault(); setHi((h) => Math.min(h + 1, options - 1)); }
                else if (e.key === "ArrowUp") { e.preventDefault(); setHi((h) => Math.max(h - 1, 0)); }
                else if (e.key === "Enter") {
                  e.preventDefault();
                  if (hi < matches.length && matches[hi]) pick(matches[hi]);
                  else if (offerNew) startNew();
                } else if (e.key === "Escape" && compact && selected) { e.stopPropagation(); setOpen(false); }
              }}
              aria-label="Kunde suchen"
            />
          </label>
          <ul className="cust-list" role="listbox">
            {matches.map((p, i) => (
              <li key={p.id}>
                <button className={`cust-opt ${i === hi ? "hi" : ""} ${p.id === value ? "on" : ""}`} onMouseEnter={() => setHi(i)} onClick={() => pick(p)}>
                  <UserRound size={14} />
                  <span className="grow">
                    <span className="cust-name">{p.name}</span>
                    <span className="muted small">{[customerNo(p.number), p.email, p.phone].filter(Boolean).join(" · ")}</span>
                  </span>
                  <span className="muted small">{p.orderCount} Aufträge</span>
                </button>
              </li>
            ))}
            {offerNew && (
              <li>
                <button className={`cust-opt new ${hi === matches.length ? "hi" : ""}`} onMouseEnter={() => setHi(matches.length)} onClick={startNew}>
                  <Plus size={14} />
                  <span className="grow">Neuen Kunden „<b>{q.trim()}</b>“ anlegen</span>
                </button>
              </li>
            )}
            {!s && people.length === 0 && (
              <li className="muted small cust-empty">Noch keine Kunden. Tippe einen Namen, um den ersten Kunden anzulegen.</li>
            )}
            {s && matches.length === 0 && !offerNew && <li className="muted small cust-empty">Kein Treffer.</li>}
          </ul>
          {compact && selected && <button className="ghost small-btn" onClick={() => { setOpen(false); setQ(""); }}><X size={12} /> Abbrechen</button>}
        </>
      )}
      {form && (
        <div className="cust-new" onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && create()}>
          <div className="cust-new-head"><Plus size={14} /> Neuer Kunde</div>
          <div className="cust-new-grid">
            <label className="block">Name<input autoFocus value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label>
            <label className="block">E-Mail (optional)<input type="email" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></label>
            <label className="block">Telefon (optional)<input value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} /></label>
          </div>
          <div className="add-row">
            <button className="primary" onClick={create} disabled={busy || !form.name.trim()}>Kunde anlegen</button>
            <button onClick={() => setForm(null)} disabled={busy}>Zurück zur Suche</button>
          </div>
        </div>
      )}
    </div>
  );
}
