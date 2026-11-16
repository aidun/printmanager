import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { api, type Person } from "./api";
import type { ViewCtx } from "./App";

export function PeopleView({ fail, notify, rev, onShowOrders }: ViewCtx & { rev: number; onShowOrders: (name: string) => void }) {
  const [people, setPeople] = useState<Person[]>([]);
  const [edit, setEdit] = useState<{ id?: string; name: string; contact: string; note: string } | null>(null);

  const load = () => api.people().then(setPeople).catch(fail);
  useEffect(() => {
    load();
  }, [rev]); // eslint-disable-line react-hooks/exhaustive-deps

  async function save() {
    if (!edit) return;
    try {
      await api.savePerson(edit);
      setEdit(null);
      load();
    } catch (e) {
      fail(e);
    }
  }

  async function remove(p: Person) {
    try {
      await api.deletePerson(p.id);
      notify(`${p.name} gelöscht`);
      load();
    } catch (e) {
      fail(e);
    }
  }

  return (
    <div className="people-view">
      <header className="toolbar">
        <h1>Personen</h1>
        <button className="primary" onClick={() => setEdit({ name: "", contact: "", note: "" })}><Plus size={14} /> Person</button>
      </header>
      {edit && (
        <div className="person-edit" onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && save()}>
          <input autoFocus autoComplete="off" placeholder="Name" value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} />
          <input autoComplete="off" placeholder="Kontakt (Telefon, E-Mail …)" value={edit.contact} onChange={(e) => setEdit({ ...edit, contact: e.target.value })} />
          <input className="grow" placeholder="Notiz" value={edit.note} onChange={(e) => setEdit({ ...edit, note: e.target.value })} />
          <button className="primary" onClick={save} disabled={!edit.name.trim()}>Speichern</button>
          <button onClick={() => setEdit(null)}>Abbrechen</button>
        </div>
      )}
      <table className="table">
        <thead>
          <tr><th>Name</th><th>Kontakt</th><th>Notiz</th><th>Aufträge</th><th /></tr>
        </thead>
        <tbody>
          {people.map((p) => (
            <tr key={p.id}>
              <td>{p.name}</td>
              <td>{p.contact}</td>
              <td className="muted">{p.note}</td>
              <td>
                {p.orderCount > 0 ? (
                  <button className="link" onClick={() => onShowOrders(p.name)}>{p.orderCount} anzeigen</button>
                ) : (
                  <span className="muted">0</span>
                )}
              </td>
              <td className="actions">
                <button className="ghost" onClick={() => setEdit({ id: p.id, name: p.name, contact: p.contact, note: p.note })}>Bearbeiten</button>
                {p.orderCount === 0 && <button className="ghost" onClick={() => remove(p)}>Löschen</button>}
              </td>
            </tr>
          ))}
          {people.length === 0 && (
            <tr><td colSpan={5} className="empty">Noch keine Personen. Sie entstehen auch automatisch beim Anlegen eines Auftrags.</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
