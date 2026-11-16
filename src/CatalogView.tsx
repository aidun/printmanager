import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Box, Link2, Plus, RefreshCw, Search, Star, X } from "lucide-react";
import { open as pickFiles } from "@tauri-apps/plugin-dialog";
import { api, catalogSrc, fmtDuration, parseFilaments, type CatalogEntry, type MwCollection } from "./api";
import { LinkImport } from "./LinkImport";

interface Props {
  dataDir: string;
  /** erhöht sich, wenn außerhalb (z.B. per Drag&Drop) importiert wurde */
  rev: number;
  notify: (t: string, error?: boolean) => void;
  fail: (e: unknown) => void;
}

export function CatalogView({ dataDir, rev, notify, fail }: Props) {
  const [all, setAll] = useState<CatalogEntry[]>([]);
  const [q, setQ] = useState("");
  const [showHidden, setShowHidden] = useState(false);
  const [edit, setEdit] = useState<CatalogEntry | null>(null);
  const [linkOpen, setLinkOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [colls, setColls] = useState<MwCollection[]>([]);
  const [mwOn, setMwOn] = useState(false);
  const [syncing, setSyncing] = useState(false);
  /** alle | lokal | makerworld | <Sammlungs-ID> */
  const [scope, setScope] = useState("alle");

  const load = useCallback(() => {
    api.catalog(true).then(setAll).catch(fail);
    api.mwCollections().then((l) => setColls(l.filter((c) => c.enabled))).catch(() => setColls([]));
    api.bambuStatus().then((b) => setMwOn(b.connected && b.makerworld)).catch(() => setMwOn(false));
  }, [fail]);
  useEffect(() => {
    load();
  }, [load, rev]);

  const syncMw = async () => {
    setSyncing(true);
    try {
      const r = await api.mwSync();
      notify(`MakerWorld abgeglichen: ${r.collections} Sammlungen, ${r.designs} Modelle${r.added ? `, ${r.added} neu` : ""}`);
      load();
    } catch (e) {
      fail(e);
    } finally {
      setSyncing(false);
    }
  };

  // Einträge ohne Datei, die in keiner Sammlung mehr sind, zählen als ausgeblendet
  const isOff = (e: CatalogEntry) => !!e.hidden || (!!e.remoteMissing && !e.fileName);
  const list = useMemo(() => {
    const s = q.trim().toLowerCase();
    return all
      .filter((e) => showHidden || !isOff(e))
      .filter((e) =>
        scope === "alle" ? true
          : scope === "lokal" ? !e.remoteId
            : scope === "makerworld" ? !!e.remoteId
              : e.collectionIds.includes(scope),
      )
      .filter((e) => !s || `${e.title} ${e.tags} ${e.designer} ${e.description}`.toLowerCase().includes(s));
  }, [all, q, showHidden, scope]);
  const count = (f: (e: CatalogEntry) => boolean) => all.filter((e) => !isOff(e) && f(e)).length;

  async function addFiles() {
    const sel = await pickFiles({
      multiple: true,
      title: "Projekte zum Katalog hinzufügen",
      filters: [{ name: "Druckdateien", extensions: ["3mf", "stl", "step", "stp", "obj"] }],
    });
    const paths = Array.isArray(sel) ? sel : sel ? [sel] : [];
    if (!paths.length) return;
    try {
      const ids = await api.catalogImportPaths(paths);
      notify(ids.length === 1 ? "Projekt hinzugefügt" : `${ids.length} Projekte hinzugefügt`);
      load();
    } catch (e) {
      fail(e);
    }
  }

  return (
    <div className="catalog-view">
      <header className="toolbar">
        <h1>Katalog</h1>
        <label className="search"><Search size={14} /><input type="search" placeholder="Katalog durchsuchen …" value={q} onChange={(e) => setQ(e.target.value)} /></label>
        <label className="check">
          <input type="checkbox" checked={showHidden} onChange={(e) => setShowHidden(e.target.checked)} /> Ausgeblendete
        </label>
        {mwOn && <button onClick={syncMw} disabled={syncing}><RefreshCw size={13} className={syncing ? "spin" : ""} /> {syncing ? "Gleicht ab …" : "MakerWorld abgleichen"}</button>}
        <button onClick={() => setLinkOpen(true)}><Link2 size={13} /> Von Link …</button>
        <button className="primary" onClick={addFiles}><Plus size={14} /> Dateien</button>
      </header>
      <p className="hint pad">
        Fertige Projekte aus Bambu Studio (3MF) oder STL/STEP. Dateien kannst du auch einfach ins Fenster ziehen.
        Aus dem Katalog wählst du beim Anlegen eines Auftrags aus.
      </p>

      <div className="cat-layout">
      {(colls.length > 0 || all.some((e) => e.remoteId)) && (
        <nav className="cat-scopes" aria-label="Sammlungen">
          <button className={scope === "alle" ? "active" : ""} onClick={() => setScope("alle")}>Alle <span>{count(() => true)}</span></button>
          <button className={scope === "lokal" ? "active" : ""} onClick={() => setScope("lokal")}>Eigene <span>{count((e) => !e.remoteId)}</span></button>
          <button className={scope === "makerworld" ? "active" : ""} onClick={() => setScope("makerworld")}>MakerWorld <span>{count((e) => !!e.remoteId)}</span></button>
          {colls.length > 0 && <div className="cat-scopes-head">Sammlungen</div>}
          {colls.map((c) => (
            <button key={c.id} className={scope === c.id ? "active" : ""} onClick={() => setScope(c.id)} title={c.title}>
              <span className="scope-title">{c.isDefault ? <Star size={12} /> : null}{c.title}</span> <span>{count((e) => e.collectionIds.includes(c.id))}</span>
            </button>
          ))}
        </nav>
      )}
      <div className="cat-grid">
        {list.map((e) => {
          const src = catalogSrc(dataDir, e);
          return (
            <article key={e.id} className={`cat ${e.hidden ? "hidden" : ""}`} onClick={() => setEdit(e)} tabIndex={0}
              onKeyDown={(ev) => ev.key === "Enter" && setEdit(e)}>
              {src ? <img src={src} alt="" loading="lazy" /> : <div className="noimg"><Box size={28} strokeWidth={1.4} /></div>}
              <div className="cat-body">
                <div className="title">{e.title}</div>
                <div className="meta">
                  <span className="swatches">
                    {parseFilaments(e.filaments).map((f, i) => (
                      <i key={i} style={{ background: f.color || "transparent" }} title={f.type} />
                    ))}
                  </span>
                  {e.printSeconds && <span>{fmtDuration(e.printSeconds)}</span>}
                  {e.weightGrams && <span>{e.weightGrams} g</span>}
                  {e.usageCount > 0 && <span title="in Aufträgen verwendet">{e.usageCount}× bestellt</span>}
                  {e.hidden ? <span>ausgeblendet</span> : null}
                </div>
                {e.remoteId && (
                  <div className="meta">
                    <span className="mw-badge">MakerWorld</span>
                    {!e.fileName && <span title="Die 3MF lädt die App, wenn du sie brauchst">Datei bei Bedarf</span>}
                    {e.remoteMissing ? <span>nicht mehr in Sammlungen</span> : null}
                  </div>
                )}
              </div>
            </article>
          );
        })}
        {list.length === 0 && <div className="empty">{all.length ? "Nichts gefunden." : "Noch keine Projekte im Katalog."}</div>}
      </div>
      </div>

      {edit && (
        <CatalogEditDialog
          entry={edit}
          dataDir={dataDir}
          onClose={() => setEdit(null)}
          onSaved={() => {
            setEdit(null);
            load();
          }}
          notify={notify}
          fail={fail}
        />
      )}

      {linkOpen && (
        <Modal title="Projekt von Link übernehmen" onClose={() => setLinkOpen(false)}>
          <p className="hint">Printables gibt Dateien direkt heraus. Bei MakerWorld lädst du die 3MF selbst herunter und ziehst sie hier hinein.</p>
          <LinkImport
            action="In den Katalog"
            printFilesOnly
            busy={busy}
            fail={fail}
            onCancel={() => setLinkOpen(false)}
            onConfirm={async ({ preview, fileIds }) => {
              setBusy(true);
              try {
                const ids = await api.catalogImportLink(preview, fileIds);
                notify(ids.length === 1 ? "Projekt übernommen" : `${ids.length} Projekte übernommen`);
                setLinkOpen(false);
                load();
              } catch (e) {
                fail(e);
              } finally {
                setBusy(false);
              }
            }}
          />
        </Modal>
      )}
    </div>
  );
}

export function Modal({ title, children, onClose, wide }: { title: string; children: React.ReactNode; onClose: () => void; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    // Cursor ins erste Eingabefeld statt auf den Schließen-Knopf
    ref.current?.querySelector<HTMLElement>("input:not([type=checkbox]), textarea")?.focus();
  }, []);
  return (
    <dialog ref={ref} className={`dialog ${wide ? "wide" : ""}`} onClose={onClose} onCancel={onClose}>
      <div className="dialog-head">
        <h2>{title}</h2>
        <button className="icon-btn close" onClick={onClose} aria-label="Schließen"><X size={16} /></button>
      </div>
      {children}
    </dialog>
  );
}

function CatalogEditDialog({
  entry, dataDir, onClose, onSaved, notify, fail,
}: { entry: CatalogEntry; dataDir: string; onClose: () => void; onSaved: () => void; notify: (t: string) => void; fail: (e: unknown) => void }) {
  const [title, setTitle] = useState(entry.title);
  const [description, setDescription] = useState(entry.description);
  const [tags, setTags] = useState(entry.tags);
  const [hidden, setHidden] = useState(!!entry.hidden);
  const [askDelete, setAskDelete] = useState(false);
  const src = catalogSrc(dataDir, entry);
  const filaments = parseFilaments(entry.filaments);

  const save = async () => {
    try {
      await api.catalogUpdate({ id: entry.id, title, description, tags, hidden: hidden ? 1 : 0 });
      onSaved();
    } catch (e) {
      fail(e);
    }
  };

  return (
    <Modal title="Katalogprojekt" onClose={onClose} wide>
      <div className="cat-edit">
        <div className="cat-edit-side">
          {src ? <img src={src} alt="" /> : <div className="noimg big"><Box size={48} strokeWidth={1.2} /></div>}
          <dl>
            <dt>Datei</dt><dd>{entry.fileName || <span className="muted">noch nicht geladen</span>}</dd>
            {entry.plates > 1 && (<><dt>Platten</dt><dd>{entry.plates}</dd></>)}
            {entry.printSeconds && (<><dt>Druckzeit</dt><dd>{fmtDuration(entry.printSeconds)}</dd></>)}
            {entry.weightGrams && (<><dt>Gewicht</dt><dd>{entry.weightGrams} g</dd></>)}
            {filaments.length > 0 && (
              <>
                <dt>Filamente</dt>
                <dd className="fil-list">
                  {filaments.map((f, i) => (
                    <span key={i}><i style={{ background: f.color }} />{f.type || "?"}{f.grams ? ` · ${f.grams.toFixed(1)} g` : ""}</span>
                  ))}
                </dd>
              </>
            )}
            {entry.designer && (<><dt>Designer</dt><dd>{entry.designer}</dd></>)}
            {entry.license && (<><dt>Lizenz</dt><dd>{entry.license}</dd></>)}
            {entry.sourceUrl && (<><dt>Quelle</dt><dd><button className="link" onClick={() => api.openUrl(entry.sourceUrl!).catch(fail)}>{entry.sourceUrl}</button></dd></>)}
            <dt>Bestellt</dt><dd>{entry.usageCount}×</dd>
          </dl>
          {!entry.fileName && entry.remoteId && (
            <p className="hint">
              Aus deinen MakerWorld-Sammlungen. Die 3MF lädt die App, wenn du sie öffnest, zu einem Auftrag hinzufügst oder
              auf „Datei holen“ klickst. Lizenz beachten: nur für deinen eigenen Gebrauch.
            </p>
          )}
          {!entry.printSeconds && /\.3mf$/i.test(entry.fileName) && (
            <p className="hint">Druckzeit und Gewicht stehen nur in gesliced gespeicherten 3MF-Dateien.</p>
          )}
        </div>
        <div className="cat-edit-main">
          <label className="block">Titel<input value={title} onChange={(e) => setTitle(e.target.value)} /></label>
          <label className="block">Beschreibung<textarea rows={8} value={description} onChange={(e) => setDescription(e.target.value)} /></label>
          <label className="block">Stichworte<input value={tags} placeholder="z.B. Geschenk, Küche, Kinder" onChange={(e) => setTags(e.target.value)} /></label>
          <label className="check"><input type="checkbox" checked={hidden} onChange={(e) => setHidden(e.target.checked)} /> Ausblenden (nicht mehr zur Auswahl anbieten)</label>
        </div>
      </div>
      <footer className="row-end">
        <button onClick={() => api.catalogOpenInStudio(entry.id).then(() => notify("Bambu Studio wird geöffnet")).catch(fail)}>
          Vorlage in Bambu Studio öffnen
        </button>
        {!entry.fileName && entry.remoteId && (
          <button onClick={() => api.catalogFetchFile(entry.id).then(() => { notify("3MF geladen"); onSaved(); }).catch(fail)}>Datei holen</button>
        )}
        {entry.remoteId && entry.sourceUrl && (
          <button className="ghost" onClick={() => api.openUrl(entry.sourceUrl!).catch(fail)}>Auf MakerWorld öffnen</button>
        )}
        <span className="spacer" />
        {entry.usageCount === 0 &&
          (askDelete ? (
            <button className="danger" onClick={() => api.catalogDelete(entry.id).then(onSaved).catch(fail)}>Wirklich löschen?</button>
          ) : (
            <button className="ghost" onClick={() => setAskDelete(true)}>Löschen</button>
          ))}
        <button onClick={onClose}>Abbrechen</button>
        <button className="primary" onClick={save} disabled={!title.trim()}>Speichern</button>
      </footer>
    </Modal>
  );
}
