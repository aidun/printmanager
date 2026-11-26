import { useEffect, useMemo, useState } from "react";
import { BookUser, Link2, Mail, MapPin, Phone, Plus, Receipt, Search, Trash2, UserRound, Users } from "lucide-react";
import { api, customerNo, EMPTY_PERSON, formatAddress, personInput, type OrderSummary, type Person, type PersonInput, type PersonLink } from "./api";
import { ContactPicker, useContactSources } from "./ContactPicker";
import { Modal } from "./Modal";
import { STATUS, count, fmtDate, fmtDateTime } from "./status";
import type { ViewCtx } from "./App";

const EMPTY = EMPTY_PERSON;

/** Kundenverwaltung: alle Kunden mit Kontakt, Aufträgen und Notizen */
export function CustomersView({ orders, fail, notify, open, rev, onNewOrder }: ViewCtx & { rev: number; onNewOrder: (customerId: string) => void }) {
  const [people, setPeople] = useState<Person[]>([]);
  const [q, setQ] = useState("");
  const [edit, setEdit] = useState<PersonInput | null>(null);
  const [fromContacts, setFromContacts] = useState(false);
  const contacts = useContactSources();

  /** Kunden aus einem Kontakt anlegen und gleich öffnen */
  const importContact = async (connector: string, externalId: string) => {
    try {
      const r = await api.personFromContact(connector, externalId);
      const ppl = await api.people();
      setPeople(ppl);
      setFromContacts(false);
      notify(r.message);
      const p = ppl.find((x) => x.id === r.personId);
      if (p) setEdit(personInput(p));
    } catch (e) {
      fail(e);
    }
  };

  const load = () => api.people().then(setPeople).catch(fail);
  useEffect(() => {
    load();
  }, [rev]); // eslint-disable-line react-hooks/exhaustive-deps

  const list = useMemo(() => {
    const s = q.trim().toLowerCase();
    return people
      .filter((p) => !s || `${p.name} ${customerNo(p.number)} ${p.email} ${p.phone} ${formatAddress(p.street, p.houseNo, p.zip, p.city)} ${p.billingName} ${p.note}`.toLowerCase().includes(s))
      .sort((a, b) => a.name.localeCompare(b.name, "de"));
  }, [people, q]);

  return (
    <div className="people-view">
      <header className="toolbar">
        <h1>Kunden</h1>
        <span className="page-sub">{count(people.length, "Kunde", "Kunden")}</span>
        <label className="search">
          <Search size={14} />
          <input type="search" placeholder="Name, Nummer, E-Mail, Telefon …" value={q} onChange={(e) => setQ(e.target.value)} />
        </label>
        {contacts && <button onClick={() => setFromContacts(true)} title="Kunden aus deinen Kontakten übernehmen"><BookUser size={14} /> Aus Kontakten …</button>}
        <button className="primary" onClick={() => setEdit({ ...EMPTY, name: q.trim() })}><Plus size={14} /> Kunde</button>
      </header>

      {people.length === 0 ? (
        <div className="empty-state">
          <Users size={28} />
          <b>Noch keine Kunden</b>
          <span>Lege den ersten Kunden an, oder direkt beim nächsten Auftrag.</span>
          <button className="primary" onClick={() => setEdit({ ...EMPTY })}><Plus size={14} /> Kunde anlegen</button>
        </div>
      ) : (
        <table className="table">
          <thead>
            <tr><th>Nr.</th><th>Name</th><th>E-Mail</th><th>Telefon</th><th>Ort</th><th className="num">Offen</th><th className="num">Aufträge</th><th>Letzter Auftrag</th></tr>
          </thead>
          <tbody>
            {list.map((p) => (
              <tr key={p.id} onClick={() => setEdit(personInput(p))}>
                <td className="nr">{customerNo(p.number)}</td>
                <td><b className="cust-cell"><UserRound size={13} /> {p.name}</b>{p.note && <div className="muted small ellipsis">{p.note}</div>}</td>
                <td className="muted">{p.email}</td>
                <td className="muted">{p.phone}</td>
                <td className="muted">{[p.zip, p.city].filter(Boolean).join(" ")}{p.billingDiffers ? <span className="tag" title="Abweichende Rechnungsadresse">Rechnung</span> : null}</td>
                <td className="num">{p.openCount || ""}</td>
                <td className="num">{p.orderCount}</td>
                <td className="muted">{fmtDate(p.lastOrderAt)}</td>
              </tr>
            ))}
            {list.length === 0 && (
              <tr><td colSpan={8} className="empty">Kein Kunde gefunden. <button className="link" onClick={() => setEdit({ ...EMPTY, name: q.trim() })}>„{q.trim()}“ als Kunden anlegen</button></td></tr>
            )}
          </tbody>
        </table>
      )}

      {fromContacts && (
        <ContactPicker
          title="Kunde aus Kontakten"
          hint="Aus dem Kontakt wird ein Kunde."
          onClose={() => setFromContacts(false)}
          onPick={(h) => importContact(h.connector, h.externalId)}
        />
      )}
      {edit && (
        <CustomerDialog
          value={edit}
          person={people.find((p) => p.id === edit.id) ?? null}
          orders={orders.filter((o) => edit.id && o.personId === edit.id)}
          onClose={() => setEdit(null)}
          onSaved={(msg) => { setEdit(null); notify(msg); load(); }}
          onOpenOrder={(id) => { setEdit(null); open(id); }}
          onNewOrder={(id) => { setEdit(null); onNewOrder(id); }}
          fail={fail}
          notify={notify}
          contacts={contacts}
        />
      )}
    </div>
  );
}

function CustomerDialog({
  value, person, orders, onClose, onSaved, onOpenOrder, onNewOrder, fail, notify, contacts,
}: {
  value: PersonInput; person: Person | null; orders: OrderSummary[];
  onClose: () => void; onSaved: (msg: string) => void; onOpenOrder: (id: string) => void; onNewOrder: (id: string) => void;
  fail: (e: unknown) => void; notify: (t: string) => void; contacts: boolean;
}) {
  const [f, setF] = useState<PersonInput>(value);
  /** Werte nach Verknüpfen oder Konfliktauflösung neu laden */
  const refresh = async (msg: string) => {
    const p = (await api.people()).find((x) => x.id === person?.id);
    if (p) setF(personInput(p));
    notify(msg);
  };
  const [askDelete, setAskDelete] = useState(false);
  const set = (patch: Partial<PersonInput>) => setF((x) => ({ ...x, ...patch }));
  const sorted = [...orders].sort((a, b) => b.number - a.number);

  const save = async () => {
    try {
      await api.savePerson(f);
      onSaved(f.id ? "Kunde gespeichert" : `Kunde „${f.name.trim()}“ angelegt`);
    } catch (e) {
      fail(e);
    }
  };

  return (
    <Modal title={person ? `${person.name} · ${customerNo(person.number)}` : "Neuer Kunde"} onClose={onClose} wide={!!person}>
      <div className={person ? "cust-dialog" : ""}>
        <div className="stack">
          <label className="block">Name<input value={f.name} onChange={(e) => set({ name: e.target.value })} autoFocus /></label>
          <div className="grid-2">
            <label className="block"><span><Mail size={11} /> E-Mail</span><input type="email" value={f.email} onChange={(e) => set({ email: e.target.value })} /></label>
            <label className="block"><span><Phone size={11} /> Telefon</span><input value={f.phone} onChange={(e) => set({ phone: e.target.value })} /></label>
          </div>
          <fieldset className="addr">
            <legend><MapPin size={12} /> Adresse</legend>
            <div className="addr-grid">
              <label className="block street">Straße<input value={f.street} onChange={(e) => set({ street: e.target.value })} autoComplete="off" /></label>
              <label className="block no">Hausnr.<input value={f.houseNo} onChange={(e) => set({ houseNo: e.target.value })} autoComplete="off" /></label>
              <label className="block zip">PLZ<input value={f.zip} onChange={(e) => set({ zip: e.target.value })} inputMode="numeric" autoComplete="off" /></label>
              <label className="block city">Ort<input value={f.city} onChange={(e) => set({ city: e.target.value })} autoComplete="off" /></label>
            </div>
          </fieldset>
          <label className="check">
            <input type="checkbox" checked={f.billingDiffers} onChange={(e) => set({ billingDiffers: e.target.checked })} />
            <Receipt size={13} /> Abweichende Rechnungsadresse
          </label>
          {f.billingDiffers && (
            <fieldset className="addr">
              <legend><Receipt size={12} /> Rechnungsadresse</legend>
              <div className="addr-grid">
                <label className="block full">Name / Firma (leer = Kundenname)<input value={f.billingName} onChange={(e) => set({ billingName: e.target.value })} placeholder={f.name} autoComplete="off" /></label>
                <label className="block street">Straße<input value={f.billingStreet} onChange={(e) => set({ billingStreet: e.target.value })} autoComplete="off" /></label>
                <label className="block no">Hausnr.<input value={f.billingHouseNo} onChange={(e) => set({ billingHouseNo: e.target.value })} autoComplete="off" /></label>
                <label className="block zip">PLZ<input value={f.billingZip} onChange={(e) => set({ billingZip: e.target.value })} inputMode="numeric" autoComplete="off" /></label>
                <label className="block city">Ort<input value={f.billingCity} onChange={(e) => set({ billingCity: e.target.value })} autoComplete="off" /></label>
              </div>
            </fieldset>
          )}
          {person && <ContactLinkBox personId={person.id} contacts={contacts} onChanged={refresh} fail={fail} />}
          <label className="block">Notiz<textarea rows={3} value={f.note} onChange={(e) => set({ note: e.target.value })} placeholder="z.B. Lieblingsfarben, Abholung, Besonderheiten" /></label>
        </div>
        {person && (
          <div className="cust-orders">
            <div className="section-head">
              <h3>Aufträge <span className="muted">{orders.length}</span></h3>
              <button className="small-btn" onClick={() => onNewOrder(person.id)}><Plus size={12} /> Neuer Auftrag</button>
            </div>
            <ul className="today-list cust-order-list">
              {sorted.map((o) => (
                <li key={o.id} className="today-row" onClick={() => onOpenOrder(o.id)} tabIndex={0} onKeyDown={(e) => e.key === "Enter" && onOpenOrder(o.id)}>
                  <span className="dot" style={{ background: STATUS[o.status].color }} />
                  <span className="nr">#{o.number}</span>
                  <span className="grow"><span className="row-title">{o.title}</span><span className="row-meta"><span>{STATUS[o.status].label}</span>{o.archivedAt && <span>Archiv</span>}</span></span>
                  <span className="due-badge">{fmtDate(o.createdAt)}</span>
                </li>
              ))}
              {orders.length === 0 && <li className="muted small cust-empty">Noch keine Aufträge.</li>}
            </ul>
          </div>
        )}
      </div>
      <footer className="row-end">
        {person && orders.length === 0 && (askDelete ? (
          <button className="danger" onClick={() => api.deletePerson(person.id).then(() => onSaved(`${person.name} gelöscht`)).catch(fail)}>
            <Trash2 size={13} /> Wirklich löschen?
          </button>
        ) : (
          <button className="ghost" onClick={() => setAskDelete(true)}><Trash2 size={13} /> Löschen</button>
        ))}
        <span className="spacer" />
        <button onClick={onClose}>Abbrechen</button>
        <button className="primary" onClick={save} disabled={!f.name.trim() || (f.billingDiffers && (!f.billingStreet.trim() || !f.billingCity.trim()))}
          title={f.billingDiffers && (!f.billingStreet.trim() || !f.billingCity.trim()) ? "Rechnungsadresse: bitte mindestens Straße und Ort angeben" : undefined}>{person ? "Speichern" : "Kunde anlegen"}</button>
      </footer>
    </Modal>
  );
}

/** Verknüpfung des Kunden mit einem Kontakt: Stand, Konflikt auflösen, verknüpfen oder lösen */
function ContactLinkBox({
  personId, contacts, onChanged, fail,
}: { personId: string; contacts: boolean; onChanged: (msg: string) => Promise<void>; fail: (e: unknown) => void }) {
  const [link, setLink] = useState<PersonLink | null | undefined>(undefined);
  const [picking, setPicking] = useState(false);
  const load = () => api.personContact(personId).then(setLink).catch(fail);
  useEffect(() => {
    load();
  }, [personId]); // eslint-disable-line react-hooks/exhaustive-deps

  const act = async (fn: () => Promise<unknown>, msg: string) => {
    try {
      await fn();
      await load();
      await onChanged(msg);
    } catch (e) {
      fail(e);
    }
  };

  if (link === undefined) return null;
  if (!link) {
    if (!contacts) return null;
    return (
      <div className="contact-link">
        <button className="small-btn" onClick={() => setPicking(true)}><BookUser size={12} /> Mit Kontakt verknüpfen …</button>
        {picking && (
          <ContactPicker
            title="Mit Kontakt verknüpfen"
            hint="Leere Felder werden aus dem Kontakt gefüllt."
            onClose={() => setPicking(false)}
            onPick={async (h) => {
              setPicking(false);
              try {
                const msg = await api.personLinkContact(personId, h.connector, h.externalId);
                await load();
                await onChanged(msg);
              } catch (e) {
                fail(e);
              }
            }}
          />
        )}
      </div>
    );
  }
  return (
    <div className={`contact-link ${link.state}`}>
      <div className="add-row">
        <span className="grow small">
          <Link2 size={12} /> Verknüpft mit <b>{link.source}</b>
          {link.lastSyncedAt && <span className="muted"> · abgeglichen {fmtDateTime(link.lastSyncedAt)}</span>}
        </span>
        <button className="ghost small-btn" onClick={() => act(() => api.personUnlinkContact(personId), "Verknüpfung gelöst")}>Lösen</button>
      </div>
      {link.state === "geloescht" && <p className="warn small">Den verknüpften Kontakt gibt es nicht mehr. Der Kunde bleibt unverändert.</p>}
      {link.state === "konflikt" && (
        <>
          <p className="warn small">Kontakt und Kunde wurden beide geändert. Welcher Wert soll gelten?</p>
          <table className="table plain small">
            <thead><tr><th /><th>Kontakt</th><th>Kunde</th></tr></thead>
            <tbody>
              {link.conflicts.map((c) => (
                <tr key={c.field}><td>{c.label}</td><td>{c.contact || "–"}</td><td>{c.customer || "–"}</td></tr>
              ))}
            </tbody>
          </table>
          <div className="add-row">
            <button className="small-btn" onClick={() => act(() => api.personContactResolve(personId, true), "Werte aus dem Kontakt übernommen")}>Kontakt übernehmen</button>
            <button className="small-btn" onClick={() => act(() => api.personContactResolve(personId, false), "Werte des Kunden behalten")}>Kunde behalten</button>
          </div>
        </>
      )}
    </div>
  );
}
