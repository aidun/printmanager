import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2 } from "lucide-react";
import { api, CHANGED, fmtGrams, type Cap, type ConnectorInfo, type ConnectorKind, type ConnectorWrite, type ContactProbe, type MwCollection } from "./api";
import { Modal } from "./Modal";
import { fmtDateTime } from "./status";

type Notify = (t: string) => void;
type Fail = (e: unknown) => void;

const KIND_LABEL: Record<ConnectorKind, string> = {
  "printer-cloud": "Druckerkonten",
  slicer: "Slicer",
  catalog: "Modelle und Links",
  contacts: "Kontakte",
  storage: "Speicher",
};
const KIND_ORDER: ConnectorKind[] = ["printer-cloud", "slicer", "catalog", "contacts", "storage"];

const CAP_LABEL: Record<Cap, string> = {
  "spools-read": "Filament abgleichen",
  "spools-write": "Änderungen an Spulen zurückschreiben",
  "printers-read": "Drucker und Druckstatus anzeigen",
  "print-history-read": "Druckverlauf abgleichen",
  "catalog-read": "Sammlungen im Katalog",
  "links-read": "Links auswerten",
  "contacts-read": "Kontakte lesen",
  "slicer-open": "Dateien im Programm öffnen",
  "slicer-slice": "Im Hintergrund slicen (für die Kalkulation)",
  "profiles-read": "Druckprofile laden",
};

/** Zusatz je Anbindung und Fähigkeit (Takt, Herkunft) */
const CAP_HINT: Record<string, string> = {
  "bambu-cloud:spools-read": "alle 15 Minuten; sonst aus spools.json",
  "bambu-cloud:spools-write": "Rest und Spulenvorrat; jedes Mal mit Nachfrage",
  "bambu-cloud:printers-read": "jede Minute, beim Drucken alle 20 s",
  "bambu-cloud:print-history-read": "alle 15 Minuten",
  "bambu-cloud:catalog-read": "MakerWorld, alle 6 Stunden; Dateien nur bei Bedarf",
  "bambu-studio:spools-read": "aus spools.json, wenn das Konto kein Filament liefert",
  "bambu-studio:slicer-slice": "startet Bambu Studio ohne Oberfläche, nur zum Berechnen; nie ein Druck; ab Werk aus",
  "printerprofiles:profiles-read": "Presets je Drucker, Düse und Material; täglich aufgefrischt",
  "web:links-read": "Seitenvorschau, Bilder und Downloads",
  "printables:links-read": "Titel, Bilder und Dateien von Printables-Links",
  "apple-contacts:contacts-read": "alle Konten der Kontakte-App; verknüpfte Kunden alle 30 Minuten abgleichen; nur lesen",
};

/** Einstellungen → Anbindungen: alle externen Dienste und Programme an einer Stelle */
export function ConnectorsSection({ notify, fail }: { notify: Notify; fail: Fail }) {
  const [list, setList] = useState<ConnectorInfo[] | null>(null);
  const [confirm, setConfirm] = useState<{ c: ConnectorInfo; cap: Cap } | null>(null);
  const load = useCallback(() => api.connectors().then(setList).catch(fail), [fail]);
  useEffect(() => {
    load();
    window.addEventListener(CHANGED, load);
    // während ein Anmeldefenster offen ist, regelmäßig nachsehen
    const t = window.setInterval(load, 3000);
    return () => {
      window.removeEventListener(CHANGED, load);
      window.clearInterval(t);
    };
  }, [load]);

  const groups = useMemo(
    () => KIND_ORDER.map((k) => [k, (list ?? []).filter((c) => c.kind === k)] as const).filter(([, l]) => l.length),
    [list],
  );
  if (!list) return null;

  const setCap = (c: ConnectorInfo, cap: Cap, on: boolean, write: boolean) => {
    if (on && write) return setConfirm({ c, cap });
    api.connectorSetCap(c.id, cap, on).then(load).catch(fail);
  };
  const anyWrite = list.some((c) => c.caps.some((x) => x.write && x.on));

  return (
    <section className="section">
      <h3>Anbindungen</h3>
      <p className="hint">
        Dienste und Programme, mit denen Printmanager zusammenarbeitet. Anmeldedaten liegen nur im Schlüsselbund des
        Systems. Ab Werk wird nur gelesen; Schreiben schaltest du je Dienst ein. Drucke startest du weiter in deinem
        Slicer oder in der App des Druckers.
      </p>
      {groups.map(([kind, items]) => (
        <div key={kind} className="conn-group">
          <h4>{KIND_LABEL[kind]}</h4>
          {items.map((c) => (
            <ConnectorCard key={c.id} c={c} onCap={setCap} reload={load} notify={notify} fail={fail} />
          ))}
        </div>
      ))}
      {anyWrite && <WriteLog connectors={list} notify={notify} fail={fail} />}
      {confirm && (
        <Modal title={`In „${confirm.c.name}“ schreiben?`} onClose={() => setConfirm(null)}>
          <p>Damit kann Printmanager Daten bei {confirm.c.name} ändern, anlegen und löschen. Andere Programme zeigen dann denselben Stand.</p>
          <ul className="hint">
            <li>Die Schnittstelle ist inoffiziell. Der Anbieter kann sie jederzeit ändern oder sperren.</li>
            <li>Vor jedem Schreiben fragt die App nach und prüft, ob sich der Eintrag dort inzwischen geändert hat.</li>
            <li>Den Rest von Spulen mit RFID pflegt das AMS, dort schreibt die App keinen Rest.</li>
            <li>Steckt die Spule in einem Drucker, der gerade druckt, wartet die Änderung bis nach dem Druck.</li>
            <li>Jede Änderung steht im Protokoll und lässt sich zurücknehmen.</li>
          </ul>
          <footer className="row-end">
            <button onClick={() => setConfirm(null)}>Abbrechen</button>
            <button
              className="primary"
              onClick={() => {
                const { c, cap } = confirm;
                setConfirm(null);
                api.connectorSetCap(c.id, cap, true).then(load).catch(fail);
              }}
            >
              Einschalten
            </button>
          </footer>
        </Modal>
      )}
    </section>
  );
}

function ConnectorCard({
  c, onCap, reload, notify, fail,
}: {
  c: ConnectorInfo; onCap: (c: ConnectorInfo, cap: Cap, on: boolean, write: boolean) => void; reload: () => void; notify: Notify; fail: Fail;
}) {
  const st = c.status;
  const login = c.auth !== "none";
  const connect = () => api.connectorConnect(c.id).then(reload).catch(fail);
  const disconnect = () =>
    api.connectorDisconnect(c.id).then(() => { notify(`Von „${c.name}“ abgemeldet`); reload(); }).catch(fail);
  const catalogOn = c.caps.some((x) => x.cap === "catalog-read" && x.on);
  const contactsOn = c.caps.some((x) => x.cap === "contacts-read" && x.on);
  const permission = c.auth === "os-permission";
  return (
    <article className={`conn ${c.enabled ? "" : "off"}`}>
      <div className="add-row">
        <span className="grow">
          <b>{c.name}</b>{" "}
          {login ? (
            st.connected ? (
              <span className="ok-badge"><CheckCircle2 size={14} /> {permission ? "Freigegeben" : "Verbunden"}</span>
            ) : st.expired ? (
              <span className="warn">Anmeldung abgelaufen</span>
            ) : (
              <span className="muted">{permission ? "Nicht freigegeben" : "Nicht verbunden"}</span>
            )
          ) : (
            <span className="muted small">ohne Anmeldung</span>
          )}
          <span className="muted small">
            {st.account ? ` · ${st.account}` : ""}
            {st.detail ? ` · ${st.detail}` : ""}
            {st.connected && st.lastOkAt ? ` · zuletzt erreicht ${fmtDateTime(st.lastOkAt)}` : ""}
          </span>
        </span>
        {login ? (
          st.connected && (c.enabled || !permission) ? (
            <button onClick={disconnect}>{permission ? "Ausschalten" : "Abmelden"}</button>
          ) : (
            <button className="primary" onClick={connect}>{permission ? "Freigeben …" : st.expired ? "Neu anmelden" : "Anmelden …"}</button>
          )
        ) : (
          <label className="check">
            <input type="checkbox" checked={c.enabled} onChange={(e) => api.connectorSetEnabled(c.id, e.target.checked).then(reload).catch(fail)} /> An
          </label>
        )}
      </div>
      {st.lastError && st.connected && <p className="warn small">{st.lastError}</p>}
      {c.caps.length > 1 || login ? (
        <div className="bambu-opts">
          {c.caps.map((x) => (
              <label key={x.cap} className="check">
                <input type="checkbox" checked={x.value} disabled={!login && !c.enabled} onChange={(e) => onCap(c, x.cap, e.target.checked, x.write)} />
                {CAP_LABEL[x.cap]}
                {CAP_HINT[`${c.id}:${x.cap}`] && <span className="muted small"> ({CAP_HINT[`${c.id}:${x.cap}`]})</span>}
              </label>
          ))}
          {c.id === "bambu-cloud" && st.connected && catalogOn && <MwCollections fail={fail} />}
          {c.kind === "contacts" && st.connected && contactsOn && <ContactsProbe id={c.id} fail={fail} />}
        </div>
      ) : null}
    </article>
  );
}

const WRITE_ACTION: Record<string, string> = { rest: "Rest", anlegen: "Angelegt", loeschen: "Gelöscht", slot: "Slot", rueckgaengig: "Zurückgenommen" };
const WRITE_STATUS: Record<string, string> = { ok: "gespeichert", konflikt: "Konflikt", fehler: "Fehler", wartet: "wartet", uebersprungen: "übersprungen", erledigt: "erledigt" };

/** Protokoll der Schreibvorgänge aller Anbindungen mit „Rückgängig“ */
function WriteLog({ connectors, notify, fail }: { connectors: ConnectorInfo[]; notify: Notify; fail: Fail }) {
  const [list, setList] = useState<ConnectorWrite[]>([]);
  const load = useCallback(() => api.connectorWrites().then(setList).catch(fail), [fail]);
  useEffect(() => {
    load();
    window.addEventListener(CHANGED, load);
    return () => window.removeEventListener(CHANGED, load);
  }, [load]);
  const name = (id: string) => connectors.find((c) => c.id === id)?.name ?? id;
  const undo = (w: ConnectorWrite) => api.connectorWriteUndo(w.id).then((r) => { notify(r.message); load(); }).catch(fail);
  const val = (v: string | null) => (v == null || v === "" ? "–" : isNaN(Number(v)) ? v : fmtGrams(Number(v)));
  return (
    <div className="write-log">
      <h4>Geschriebene Änderungen</h4>
      {list.length === 0 ? (
        <p className="muted small">Noch nichts geschrieben. Änderst du den Rest einer Spule oder ziehst Verbrauch ab, fragt die App, ob er ins Konto soll.</p>
      ) : (
        <table className="table plain">
          <tbody>
            {list.map((w) => (
              <tr key={w.id}>
                <td className="muted small nowrap">{fmtDateTime(w.createdAt)}</td>
                <td className="muted small">{name(w.connector)}</td>
                <td>{w.label}</td>
                <td className="small">{WRITE_ACTION[w.action] ?? w.action}{w.oldValue || w.newValue ? `: ${val(w.oldValue)} → ${val(w.newValue)}` : ""}</td>
                <td className={`small push-msg ${w.status}`} title={w.message}>{WRITE_STATUS[w.status] ?? w.status}</td>
                <td className="num">
                  {w.action === "rest" && w.status === "ok" && !!w.oldValue && (
                    <button className="ghost small-btn" onClick={() => undo(w)} title="Alten Wert zurückschreiben">Rückgängig</button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

/** Stufe K0: Kontakte einer Quelle prüfen – nur Zahlen und Formate, keine Namen */
function ContactsProbe({ id, fail }: { id: string; fail: Fail }) {
  const [r, setR] = useState<ContactProbe | null>(null);
  const [busy, setBusy] = useState(false);
  const run = () => {
    setBusy(true);
    api.contactsProbe(id).then(setR).catch(fail).finally(() => setBusy(false));
  };
  const syncNow = () => {
    setBusy(true);
    api
      .contactsSyncNow()
      .then((m) => setNote(m.length ? m.join(" · ") : "Verknüpfte Kunden sind aktuell."))
      .catch(fail)
      .finally(() => setBusy(false));
  };
  const [note, setNote] = useState<string | null>(null);
  const list = (m: Record<string, number>) => Object.entries(m).map(([k, v]) => `${k} ${v}`).join(" · ") || "–";
  return (
    <div className="probe">
      <div className="add-row">
        <button className="small-btn" onClick={syncNow} disabled={busy}>Verknüpfte Kunden abgleichen</button>
        <button className="small-btn" onClick={run} disabled={busy}>{busy ? "Liest …" : "Kontakte prüfen"}</button>
        <span className="muted small">Prüfen zählt nur und zeigt keine Namen</span>
      </div>
      {note && <p className="muted small">{note}</p>}
      {r && (
        <table className="table plain small">
          <tbody>
            <tr><td>Kontakte</td><td className="num">{r.count}</td><td className="muted">mit Name {r.withName} · nur Firma {r.onlyOrganization}</td></tr>
            <tr><td>mit E-Mail / Telefon / Adresse</td><td className="num">{r.withEmail} / {r.withPhone} / {r.withAddress}</td><td /></tr>
            <tr><td>Straße mit / ohne Hausnummer</td><td className="num">{r.streetWithNumber} / {r.streetWithoutNumber}</td><td className="muted">mehrzeilig {r.streetMultiline}</td></tr>
            <tr><td>Telefonformate</td><td colSpan={2} className="muted">{list(r.phoneFormats)}</td></tr>
            <tr><td>Arten</td><td colSpan={2} className="muted">{list(r.kinds)}</td></tr>
            <tr><td>Länder</td><td colSpan={2} className="muted">{list(r.countries)}</td></tr>
          </tbody>
        </table>
      )}
    </div>
  );
}

/** Auswahl, welche MakerWorld-Sammlungen in den Katalog kommen */
function MwCollections({ fail }: { fail: Fail }) {
  const [list, setList] = useState<MwCollection[]>([]);
  const load = useCallback(() => api.mwCollections().then(setList).catch(fail), [fail]);
  useEffect(() => {
    load();
  }, [load]);
  if (!list.length) return <p className="muted small mw-colls">Sammlungen erscheinen nach dem ersten Abgleich (Katalog → „MakerWorld“).</p>;
  return (
    <div className="mw-colls">
      {list.map((c) => (
        <label key={c.id} className="check small">
          <input type="checkbox" checked={!!c.enabled} onChange={(e) => api.mwCollectionToggle(c.id, e.target.checked).then(load).catch(fail)} />
          {c.isDefault ? "★ " : ""}{c.title} <span className="muted">({c.designCnt})</span>
        </label>
      ))}
    </div>
  );
}
