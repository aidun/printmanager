import { useCallback, useEffect, useState } from "react";
import { open as pickFiles } from "@tauri-apps/plugin-dialog";
import { Archive, ArchiveRestore, ArrowRight, Box, Calculator, ExternalLink, FileText, FolderOpen, Link2, Quote, Trash2, X } from "lucide-react";
import { EstimateTag, ItemCalcDialog, defaultChoice, refreshOwnModels } from "./ItemCalc";
import {
  api, fileSrc, isModelFile, isModelPlatform, isUrl, plateSrc, type ItemPlates,
  type AppInfo, type Attachment, type CatalogEntry, type Item, type ItemEstimate, type OrderDetail, type Person, type Spool, type Status,
} from "./api";
import { PrintRuns, SpoolDot } from "./Prints";
import { CustomerPicker } from "./CustomerPicker";
import { CostBox, usePricing } from "./CostBox";
import { orderUsage } from "./pricing";
import { Modal } from "./Modal";
import { CatalogPicker, type CatalogPick } from "./CatalogPicker";
import { LinkImport } from "./LinkImport";
import { ALL_STATUSES, BOARD_STATUSES, PRIORITY, STATUS, fmtDateTime } from "./status";

interface Props {
  id: string;
  info: AppInfo;
  rev: number;
  onClose: () => void;
  onChanged: () => void;
  openInStudio: (attachmentId: string) => void;
  notify: (t: string, error?: boolean) => void;
  fail: (e: unknown) => void;
}

export function OrderPanel({ id, info, rev, onClose, onChanged, openInStudio, notify, fail }: Props) {
  const [d, setD] = useState<OrderDetail | null>(null);
  const [people, setPeople] = useState<Person[]>([]);
  const [spools, setSpools] = useState<Spool[]>([]);
  const [linkUrl, setLinkUrl] = useState<string | null>(null);
  const [catalogOpen, setCatalogOpen] = useState(false);
  const [catalog, setCatalog] = useState<Map<string, CatalogEntry>>(new Map());
  const [estimates, setEstimates] = useState<ItemEstimate[]>([]);
  const pricing = usePricing(rev);

  const load = useCallback(() => {
    api.order(id).then(setD).catch(fail);
    api.itemEstimates(id).then(setEstimates).catch(() => setEstimates([]));
    api.catalog(true).then((l) => setCatalog(new Map(l.map((e) => [e.id, e])))).catch(() => {});
    api.people().then(setPeople).catch(fail);
    api.spools().then(setSpools).catch(fail);
  }, [id, fail]);

  useEffect(load, [load, rev]);

  /** Aktion ausführen, danach Detail und Übersicht neu laden */
  const run = useCallback(
    async (fn: () => Promise<unknown>, ok?: string) => {
      try {
        await fn();
        if (ok) notify(ok);
        onChanged();
      } catch (e) {
        fail(e);
      }
    },
    [notify, onChanged, fail],
  );

  // Einfügen (⌘V) außerhalb von Eingabefeldern: Bilder, Links oder Text anhängen
  useEffect(() => {
    const onPaste = async (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
      const dt = e.clipboardData;
      if (!dt) return;
      const filesIn = Array.from(dt.files);
      e.preventDefault();
      if (filesIn.length) {
        for (const f of filesIn) {
          const ext = f.type.split("/")[1] || "png";
          const name = f.name && f.name !== "image.png" ? f.name : `eingefuegt-${Date.now()}.${ext}`;
          await run(async () => api.addBytes(id, name, new Uint8Array(await f.arrayBuffer())), "Bild eingefügt");
        }
        return;
      }
      const text = dt.getData("text/plain").trim();
      if (!text) return;
      if (isUrl(text) && isModelPlatform(text)) setLinkUrl(text);
      else if (isUrl(text)) await run(() => api.addLink(id, text), "Link eingefügt");
      else await run(() => api.addText(id, text), "Text eingefügt");
    };
    document.addEventListener("paste", onPaste);
    return () => document.removeEventListener("paste", onPaste);
  }, [id, run]);

  if (!d) return <aside className="panel loading">Lädt …</aside>;
  const o = d.order;

  const save = (patch: Partial<{ title: string; description: string; personId: string | null; priority: number; dueDate: string | null }>) =>
    run(() =>
      api.updateOrder({
        id: o.id, title: o.title, description: o.description, personId: o.personId,
        priority: o.priority, dueDate: o.dueDate, ...patch,
      }),
    );

  const nextStatus = BOARD_STATUSES[BOARD_STATUSES.indexOf(o.status) + 1] as Status | undefined;
  // neueste Druckdatei zuerst (z.B. die in Bambu Studio gesliced gespeicherte Version)
  const printFiles = d.attachments.filter((a) => a.kind === "file" && isModelFile(a.name)).reverse();

  const usage = orderUsage(d, catalog, estimates);
  return (
    <aside className="panel" aria-label={`Auftrag ${o.number}`}>

      <header className="panel-head">
        <span className="nr">Auftrag #{o.number}</span>
        <span className="status-pill" style={{ ["--c" as string]: STATUS[o.status].color }}>{STATUS[o.status].label}</span>
        {o.archivedAt && <span className="tag">Archiv</span>}
        <span className="spacer" />
        {printFiles.length > 0 && (
          <button className="primary small-btn" onClick={() => openInStudio(printFiles[0].id)} title={`${printFiles[0].name} in Bambu Studio öffnen`}>
            <ExternalLink size={13} /> In Bambu Studio öffnen
          </button>
        )}
        <button className="icon-btn" onClick={() => run(() => api.revealOrder(o.id))} title="Auftragsordner öffnen" aria-label="Auftragsordner öffnen">
          <FolderOpen size={16} />
        </button>
        {(o.archivedAt || o.status === "uebergeben" || o.status === "abgelehnt") && (
          <button
            className="icon-btn"
            onClick={() => run(() => api.archive(o.id, !o.archivedAt), o.archivedAt ? "Aus dem Archiv geholt" : "Archiviert")}
            title={o.archivedAt ? "Aus dem Archiv holen" : "Archivieren: vom Board nehmen; bleibt in Suche, Liste „Archiv“ und Statistik"}
            aria-label={o.archivedAt ? "Aus dem Archiv holen" : "Archivieren"}
          >
            {o.archivedAt ? <ArchiveRestore size={16} /> : <Archive size={16} />}
          </button>
        )}
        <DeleteButton onDelete={() => run(async () => { await api.deleteOrder(o.id); onClose(); }, "Auftrag gelöscht")} />
        <span className="head-sep" />
        <button className="icon-btn" onClick={onClose} title="Schließen (Esc)" aria-label="Schließen"><X size={17} /></button>
      </header>

      <div className="panel-body">
        <input
          className="title-input"
          defaultValue={o.title}
          key={`t-${o.updatedAt}`}
          onBlur={(e) => e.target.value.trim() !== o.title && save({ title: e.target.value })}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          aria-label="Titel"
        />

        <dl className="props">
          <dt>Status</dt>
          <dd className="status-row">
            <select
              value={o.status}
              onChange={(e) => run(() => api.setStatus(o.id, e.target.value as Status))}
              style={{ ["--c" as string]: STATUS[o.status].color }}
              className="status-select"
              aria-label="Status"
            >
              {ALL_STATUSES.map((s) => (
                <option key={s} value={s}>{STATUS[s].label}</option>
              ))}
            </select>
            {nextStatus && (
              <button className="small-btn" onClick={() => run(() => api.setStatus(o.id, nextStatus))} title={STATUS[nextStatus].hint}>
                Weiter: {STATUS[nextStatus].label} <ArrowRight size={13} />
              </button>
            )}
            <span className="muted small">{STATUS[o.status].hint}</span>
          </dd>
          <dt>Kunde</dt>
          <dd className="cust-dd">
            <CustomerPicker
              compact
              people={people}
              value={o.personId}
              fail={fail}
              onChange={(pid, created) => {
                if (created) setPeople((l) => [...l.filter((p) => p.id !== pid), created]);
                save({ personId: pid });
              }}
            />
          </dd>
          <dt>Fällig bis</dt>
          <dd>
            <input
              type="date"
              defaultValue={o.dueDate ?? ""}
              key={`d-${o.updatedAt}`}
              onChange={(e) => save({ dueDate: e.target.value || null })}
              aria-label="Fällig bis"
            />
          </dd>
          <dt>Priorität</dt>
          <dd>
            <select value={o.priority} onChange={(e) => save({ priority: Number(e.target.value) })} aria-label="Priorität">
              {PRIORITY.map((p, i) => (
                <option key={p} value={i}>{p}</option>
              ))}
            </select>
          </dd>
        </dl>

        <CostBox usage={usage} pricing={pricing} />

        <label className="block">
          Beschreibung
          <textarea
            rows={4}
            defaultValue={o.description}
            key={`b-${o.updatedAt}`}
            placeholder="Was genau soll gedruckt werden? Größe, Material, Anlass …"
            onBlur={(e) => e.target.value.trim() !== o.description && save({ description: e.target.value })}
          />
        </label>

        <Items items={d.items} attachments={d.attachments} orderId={o.id} run={run} openInStudio={openInStudio}
          onCatalog={() => setCatalogOpen(true)} spools={spools} estimates={estimates} catalog={catalog} orderBasis={usage?.basis}
          onEstimate={(e) => setEstimates((l) => [...l.filter((x) => x.itemId !== e.itemId), e])} fail={fail} />

        <PrintRuns d={d} spools={spools} run={run} notify={notify} />

        <Attachments d={d} run={run} openInStudio={openInStudio} onLink={() => setLinkUrl("")} />

        <History d={d} run={run} />
      </div>

      {linkUrl !== null && (
        <LinkDialog orderId={o.id} initialUrl={linkUrl} onClose={() => setLinkUrl(null)} run={run} fail={fail} />
      )}
      {catalogOpen && (
        <CatalogDialog orderId={o.id} dataDir={info.dataDir} onClose={() => setCatalogOpen(false)} run={run} fail={fail} />
      )}
    </aside>
  );
}

function LinkDialog({ orderId, initialUrl, onClose, run, fail }: { orderId: string; initialUrl: string; onClose: () => void; run: Run; fail: (e: unknown) => void }) {
  const [busy, setBusy] = useState(false);
  return (
    <Modal title="Link übernehmen" onClose={onClose}>
      <LinkImport
        initialUrl={initialUrl}
        action="Übernehmen"
        busy={busy}
        fail={fail}
        onCancel={onClose}
        onConfirm={async ({ preview, fileIds }) => {
          setBusy(true);
          await run(() => api.linkImport(orderId, preview, fileIds), fileIds.length ? "Link und Dateien übernommen" : "Link übernommen");
          setBusy(false);
          onClose();
        }}
      />
    </Modal>
  );
}

function CatalogDialog({ orderId, dataDir, onClose, run, fail }: { orderId: string; dataDir: string; onClose: () => void; run: Run; fail: (e: unknown) => void }) {
  const [picks, setPicks] = useState<CatalogPick[]>([]);
  return (
    <Modal title="Aus dem Katalog hinzufügen" onClose={onClose} wide>
      <CatalogPicker dataDir={dataDir} picks={picks} onChange={setPicks} fail={fail} />
      <footer className="row-end">
        <button onClick={onClose}>Abbrechen</button>
        <button
          className="primary"
          disabled={!picks.length}
          onClick={async () => {
            await run(async () => {
              for (const p of picks) await api.catalogAddToOrder(orderId, p.entry.id, p.quantity, p.colorWish);
            }, picks.length === 1 ? "Projekt hinzugefügt" : `${picks.length} Projekte hinzugefügt`);
            onClose();
          }}
        >
          Hinzufügen
        </button>
      </footer>
    </Modal>
  );
}

function DeleteButton({ onDelete }: { onDelete: () => void }) {
  const [ask, setAsk] = useState(false);
  useEffect(() => {
    if (!ask) return;
    const t = window.setTimeout(() => setAsk(false), 4000);
    return () => window.clearTimeout(t);
  }, [ask]);
  return ask ? (
    <button className="danger small-btn" onClick={onDelete}><Trash2 size={13} /> Wirklich löschen?</button>
  ) : (
    <button className="icon-btn" onClick={() => setAsk(true)} title="Auftrag löschen" aria-label="Auftrag löschen"><Trash2 size={16} /></button>
  );
}


type Run = (fn: () => Promise<unknown>, ok?: string) => Promise<void>;

function Items({
  items, attachments, orderId, run, openInStudio, onCatalog, spools, estimates, catalog, onEstimate, fail, orderBasis,
}: {
  items: Item[]; attachments: Attachment[]; orderId: string; run: Run; openInStudio: (id: string) => void;
  onCatalog: () => void; spools: Spool[]; estimates: ItemEstimate[]; catalog: Map<string, CatalogEntry>;
  onEstimate: (e: ItemEstimate) => void; fail: (e: unknown) => void; orderBasis?: string;
}) {
  const [calcFor, setCalcFor] = useState<Item | null>(null);
  const [allBusy, setAllBusy] = useState<string | null>(null);
  const [plates, setPlates] = useState<ItemPlates[]>([]);
  // Platten der Projektdateien; neu laden, wenn sich Teile oder Dateien ändern
  const platesKey = items.map((i) => `${i.id}:${i.fileAttachmentId ?? ""}:${i.catalogId ?? ""}`).join("|") + `#${attachments.length}`;
  useEffect(() => {
    let alive = true;
    api.orderPlates(orderId).then((l) => alive && setPlates(l)).catch(() => alive && setPlates([]));
    return () => {
      alive = false;
    };
  }, [orderId, platesKey]);
  /** Platte an- oder abwählen; die Schätzung rechnet sofort neu, wenn sie Werte je Platte hat */
  const togglePlate = async (p: ItemPlates, index: number) => {
    const all = p.plates.map((x) => x.index);
    const cur = p.chosen.length ? p.chosen : all;
    let next = cur.includes(index) ? cur.filter((x) => x !== index) : [...cur, index].sort((a, b) => a - b);
    if (next.length === 0) return; // mindestens eine Platte
    if (next.length === all.length) next = [];
    setPlates((l) => l.map((x) => (x.itemId === p.itemId ? { ...x, chosen: next } : x)));
    try {
      onEstimate(await api.itemSetPlates(p.itemId, next));
    } catch (e) {
      fail(e);
    }
  };
  const [allMsg, setAllMsg] = useState<string | null>(null);

  /** alle Teile nacheinander slicen und kalkulieren; Teile ohne Profil bekommen die Standardauswahl */
  const sliceAll = async () => {
    setAllMsg(null);
    const done: string[] = [];
    const skipped: string[] = [];
    const errors: string[] = [];
    try {
      const [cat, own] = await Promise.all([api.sliceProfiles(), refreshOwnModels()]);
      for (const [i, it] of items.entries()) {
        setAllBusy(`Slict ${i + 1} von ${items.length}: ${it.title}`);
        const setup = await api.itemSetup(it.id);
        if (!setup.layout) {
          skipped.push(it.title);
          continue;
        }
        const choice = defaultChoice(it, setup, cat, spools, own);
        if (choice) await api.itemSetProfile(it.id, choice);
        try {
          onEstimate(await api.itemSlice(it.id));
          done.push(it.title);
        } catch (e) {
          const msg = String(e);
          // ausgeschaltet oder kein Slicer: gilt für alle Teile, also abbrechen
          if (/ausgeschaltet|Kein Slicer|nicht gefunden/.test(msg)) throw e;
          errors.push(`${it.title}: ${msg}`);
        }
      }
      setAllMsg([
        done.length ? `${done.length} ${done.length === 1 ? "Teil" : "Teile"} kalkuliert.` : "",
        skipped.length ? `Ohne Modelldatei übersprungen: ${skipped.join(", ")}.` : "",
        errors.length ? `Fehler bei ${errors.length} ${errors.length === 1 ? "Teil" : "Teilen"} (Details im Dialog des Teils).` : "",
      ].filter(Boolean).join(" "));
    } catch (e) {
      fail(e);
    } finally {
      setAllBusy(null);
    }
  };
  const [title, setTitle] = useState("");
  const [qty, setQty] = useState(1);
  const [color, setColor] = useState("");

  const add = async () => {
    if (!title.trim()) return;
    await run(() => api.saveItem({ orderId, title, quantity: qty, colorWish: color }));
    setTitle("");
    setQty(1);
    setColor("");
  };

  return (
    <section className="section">
      <div className="section-head">
        <h3>Teile <span className="muted small">{items.filter((i) => i.done).length}/{items.length} erledigt</span></h3>
        <div className="section-actions">
          {items.length > 0 && (
            <button className="ghost" onClick={sliceAll} disabled={!!allBusy}
              title={orderBasis === "tatsaechlich" ? "Der Auftrag ist schon gedruckt; die Kalkulation nutzt die tatsächlichen Werte." : "Alle Teile im Hintergrund slicen und kalkulieren"}>
              <Calculator size={13} /> {allBusy ? "Slict …" : "Alle kalkulieren"}
            </button>
          )}
          <button className="ghost" onClick={onCatalog}>+ Aus Katalog</button>
        </div>
      </div>
      {allBusy && <p className="muted small">{allBusy}</p>}
      {allMsg && !allBusy && <p className="muted small">{allMsg}</p>}
      {items.length > 0 && (
        <ul className="items">
          {items.map((it) => (
            <li key={it.id} className={it.done ? "done" : ""}>
              <input
                type="checkbox"
                checked={!!it.done}
                onChange={(e) => run(() => api.saveItem({ ...it, done: e.target.checked ? 1 : 0 }))}
                aria-label="erledigt"
              />
              <input
                className="grow"
                defaultValue={it.title}
                onBlur={(e) => e.target.value !== it.title && run(() => api.saveItem({ ...it, title: e.target.value }))}
                aria-label="Bezeichnung"
              />
              <input
                type="number"
                min={1}
                className="qty"
                defaultValue={it.quantity}
                onBlur={(e) => Number(e.target.value) !== it.quantity && run(() => api.saveItem({ ...it, quantity: Number(e.target.value) }))}
                aria-label="Anzahl"
              />
              <input
                className="color"
                defaultValue={it.colorWish}
                placeholder="Farbe"
                onBlur={(e) => e.target.value !== it.colorWish && run(() => api.saveItem({ ...it, colorWish: e.target.value }))}
                aria-label="Farbwunsch"
              />
              <SpoolDot wish={it.colorWish} spools={spools} />
              {it.fileAttachmentId && attachments.some((a) => a.id === it.fileAttachmentId) ? (
                <button className="ghost icon-btn studio" title={`${attachments.find((a) => a.id === it.fileAttachmentId)?.name} in Bambu Studio öffnen`}
                  onClick={() => openInStudio(it.fileAttachmentId!)} aria-label="In Bambu Studio öffnen"><ExternalLink size={14} /></button>
              ) : <span className="icon-spacer" />}
              <button className="ghost icon-btn" onClick={() => setCalcFor(it)} aria-label="Kalkulation" title="Kalkulation vorab (Slicer-Profil oder MakerWorld)"><Calculator size={14} /></button>
              <button className="ghost icon-btn" onClick={() => run(() => api.deleteItem(it.id))} aria-label="Teil entfernen"><X size={14} /></button>
              <EstimateTag e={estimates.find((e) => e.itemId === it.id)} />
              {(() => {
                const p = plates.find((x) => x.itemId === it.id);
                if (!p) return null;
                const on = (i: number) => p.chosen.length === 0 || p.chosen.includes(i);
                const n = p.plates.filter((x) => on(x.index)).length;
                return (
                  <div className="item-plates">
                    <div className="item-plates-head muted small">
                      <span className="grow" title={p.fileName}>{p.fileName}</span>
                      <span>{n === p.plates.length ? `alle ${n} Platten` : `${n} von ${p.plates.length} Platten`}</span>
                    </div>
                    <ul className="plate-cards">
                      {p.plates.map((x) => (
                        <li key={x.index}>
                          <label className={`plate-card ${on(x.index) ? "" : "off"}`} title={on(x.index) ? "Wird gedruckt – zum Abwählen klicken" : "Wird nicht gedruckt – zum Auswählen klicken"}>
                            <input type="checkbox" checked={on(x.index)} onChange={() => togglePlate(p, x.index)}
                              aria-label={`Platte ${x.index}${x.name ? ` ${x.name}` : ""} drucken`} />
                            {x.image ? <img src={plateSrc(x.image)} alt="" loading="lazy" /> : <span className="plate-noimg">Kein Bild</span>}
                            <span className="plate-label">Platte {x.index}{x.name ? ` · ${x.name}` : ""}</span>
                          </label>
                        </li>
                      ))}
                    </ul>
                  </div>
                );
              })()}
            </li>
          ))}
        </ul>
      )}
      {calcFor && (
        <ItemCalcDialog
          item={calcFor}
          estimate={estimates.find((e) => e.itemId === calcFor.id)}
          entry={calcFor.catalogId ? catalog.get(calcFor.catalogId) : undefined}
          spools={spools}
          orderBasis={orderBasis}
          onClose={() => setCalcFor(null)}
          onChanged={onEstimate}
          fail={fail}
        />
      )}
      <div className="add-row" onKeyDown={(e) => e.key === "Enter" && add()}>
        <input className="grow" placeholder="Teil hinzufügen, z.B. „Wandhalter Switch“" value={title} onChange={(e) => setTitle(e.target.value)} />
        <input type="number" min={1} className="qty" value={qty} onChange={(e) => setQty(Number(e.target.value))} aria-label="Anzahl" />
        <input className="color" placeholder="Farbe" value={color} onChange={(e) => setColor(e.target.value)} />
        <button onClick={add} disabled={!title.trim()}>Hinzufügen</button>
      </div>
    </section>
  );
}

function Attachments({ d, run, openInStudio, onLink }: { d: OrderDetail; run: Run; openInStudio: (id: string) => void; onLink: () => void }) {
  const [mode, setMode] = useState<null | "text">(null);
  const [value, setValue] = useState("");
  const id = d.order.id;
  const images = d.attachments.filter((a) => a.kind === "image");
  const others = d.attachments.filter((a) => a.kind !== "image");

  const submit = async () => {
    if (!value.trim()) return;
    await run(() => api.addText(id, value));
    setValue("");
    setMode(null);
  };

  const choose = async () => {
    const sel = await pickFiles({ multiple: true, title: "Dateien anhängen" });
    const paths = Array.isArray(sel) ? sel : sel ? [sel] : [];
    if (paths.length) await run(() => api.addPaths(id, paths), "Angehängt");
  };

  return (
    <section className="section">
      <h3>Anhänge</h3>
      <p className="hint">Dateien ins Fenster ziehen oder mit ⌘V Bilder, Links und Text einfügen.</p>
      <div className="att-actions">
        <button onClick={choose}>Datei wählen …</button>
        <button onClick={onLink}>Link …</button>
        <button onClick={() => setMode(mode === "text" ? null : "text")}>Text</button>
      </div>
      {mode && (
        <div className="att-input">
          <textarea autoFocus rows={4} placeholder="z.B. Nachricht aus WhatsApp" value={value} onChange={(e) => setValue(e.target.value)} />
          <button className="primary" onClick={submit} disabled={!value.trim()}>Speichern</button>
        </div>
      )}

      {images.length > 0 && (
        <div className="thumbs">
          {images.map((a) => (
            <figure key={a.id}>
              <img src={fileSrc(d.folder, a.path!)} alt={a.name} onClick={() => run(() => api.openAttachment(a.id))} title="Öffnen" />
              <button className="ghost icon-btn del" onClick={() => run(() => api.deleteAttachment(a.id))} aria-label="Bild entfernen"><X size={14} /></button>
            </figure>
          ))}
        </div>
      )}

      {others.length > 0 && (
        <ul className="att-list">
          {others.map((a) => <AttachmentRow key={a.id} a={a} run={run} openInStudio={openInStudio} />)}
        </ul>
      )}
    </section>
  );
}

function AttachmentRow({ a, run, openInStudio }: { a: Attachment; run: Run; openInStudio: (id: string) => void }) {
  return (
    <li className={`att ${a.kind}`}>
      {a.kind === "link" && (
        <>
          <span className="att-icon" aria-hidden><Link2 size={15} /></span>
          <button className="link grow" onClick={() => run(() => api.openUrl(a.url!))} title={a.url ?? ""}>
            {a.name}
          </button>
        </>
      )}
      {a.kind === "text" && (
        <>
          <span className="att-icon" aria-hidden><Quote size={14} /></span>
          <blockquote className="grow">{a.body}</blockquote>
        </>
      )}
      {a.kind === "file" && (
        <>
          <span className="att-icon" aria-hidden>{isModelFile(a.name) ? <Box size={15} /> : <FileText size={15} />}</span>
          <span className="grow">{a.name}</span>
          {isModelFile(a.name) && (
            <button className="primary" onClick={() => openInStudio(a.id)}>
              In Bambu Studio öffnen
            </button>
          )}
          <button onClick={() => run(() => api.openAttachment(a.id))}>Öffnen</button>
        </>
      )}
      <button className="ghost icon-btn" onClick={() => run(() => api.deleteAttachment(a.id))} aria-label="Anhang entfernen"><X size={14} /></button>
    </li>
  );
}

function History({ d, run }: { d: OrderDetail; run: Run }) {
  const [note, setNote] = useState("");
  const add = async () => {
    if (!note.trim()) return;
    await run(() => api.addNote(d.order.id, note));
    setNote("");
  };
  return (
    <section className="section">
      <h3>Verlauf</h3>
      <div className="add-row">
        <input
          className="grow"
          placeholder="Notiz, z.B. „Farbe mit Oma abgesprochen: rot“"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <button onClick={add} disabled={!note.trim()}>Notiz</button>
      </div>
      <ol className="events">
        {d.events.map((ev) => (
          <li key={ev.id}>
            <time>{fmtDateTime(ev.createdAt)}</time>
            {ev.kind === "created" && <span>Auftrag angelegt</span>}
            {ev.kind === "status" && (
              <span>
                {ev.fromStatus ? STATUS[ev.fromStatus].label : "?"} → <b>{ev.toStatus ? STATUS[ev.toStatus].label : "?"}</b>
                {ev.body && <> · {ev.body}</>}
              </span>
            )}
            {ev.kind === "note" && <span className="note">{ev.body}</span>}
            {ev.kind === "print" && <span className="print-ev">{ev.body}</span>}
          </li>
        ))}
      </ol>
    </section>
  );
}
