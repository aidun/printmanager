import { useEffect, useRef, useState } from "react";
import { BookUser, Link2, Search } from "lucide-react";
import { api, CHANGED, type ContactHit } from "./api";
import { Modal } from "./Modal";

/** Gibt es eine eingeschaltete, verbundene Kontaktquelle? */
export function useContactSources(): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => {
    const load = () =>
      api
        .connectors()
        .then((l) => setOn(l.some((c) => c.kind === "contacts" && c.status.connected && c.caps.some((x) => x.cap === "contacts-read" && x.on))))
        .catch(() => setOn(false));
    load();
    window.addEventListener(CHANGED, load);
    return () => window.removeEventListener(CHANGED, load);
  }, []);
  return on;
}

/**
 * Kontakt aus den eingeschalteten Kontaktquellen suchen und wählen (z.B. Apple Kontakte).
 * Die App liest nur; Kontakte werden nie verändert.
 */
export function ContactPicker({
  title, hint, onPick, onClose,
}: { title: string; hint: string; onPick: (h: ContactHit) => Promise<void> | void; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<ContactHit[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const seq = useRef(0);

  useEffect(() => {
    const n = ++seq.current;
    const t = window.setTimeout(() => {
      api
        .contactsSearch(q)
        .then((h) => n === seq.current && (setHits(h), setError(null)))
        .catch((e) => n === seq.current && setError(String(e)));
    }, q ? 200 : 0);
    return () => window.clearTimeout(t);
  }, [q]);

  const pick = async (h: ContactHit) => {
    setBusy(true);
    try {
      await onPick(h);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title={title} onClose={onClose}>
      <p className="hint">{hint}</p>
      <label className="search contact-search">
        <Search size={14} />
        <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Name, Firma, E-Mail oder Telefon …" aria-label="Kontakt suchen" />
      </label>
      {error && <p className="warn small">{error}</p>}
      <ul className="contact-list">
        {hits?.map((h) => (
          <li key={`${h.connector}:${h.externalId}`}>
            <button className="cust-opt" disabled={busy} onClick={() => pick(h)}>
              <BookUser size={14} />
              <span className="grow">
                <span className="cust-name">{h.name}</span>
                <span className="muted small">{[h.organization !== h.name ? h.organization : "", h.phone, h.email, h.city].filter(Boolean).join(" · ")}</span>
              </span>
              {h.personId ? <span className="muted small"><Link2 size={11} /> schon Kunde</span> : <span className="muted small">{h.source}</span>}
            </button>
          </li>
        ))}
        {hits && hits.length === 0 && <li className="muted small cust-empty">Kein Kontakt gefunden.</li>}
        {!hits && !error && <li className="muted small cust-empty">Kontakte werden gelesen …</li>}
      </ul>
    </Modal>
  );
}
