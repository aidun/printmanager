import { useEffect, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";
import { open as pickFiles } from "@tauri-apps/plugin-dialog";
import { api, customerNo, isUrl, type Person } from "./api";
import { CustomerPicker } from "./CustomerPicker";
import { CatalogPicker, type CatalogPick } from "./CatalogPicker";
import { LinkImport, type LinkChoice } from "./LinkImport";
import { PRIORITY } from "./status";
import { CostBox, usePricing } from "./CostBox";
import { catalogUsage } from "./pricing";

interface Props {
  dataDir: string;
  /** Dateien, die ins Fenster gezogen wurden */
  paths: string[];
  /** Hier meldet sich der Assistent für weitere Drag&Drops an */
  dropRef: React.MutableRefObject<((paths: string[]) => void) | null>;
  onClose: () => void;
  onCreated: (id: string) => void;
  fail: (e: unknown) => void;
  /** Kunde schon gewählt (z.B. „Neuer Auftrag“ beim Kunden): Assistent startet bei „Was?“ */
  customerId?: string;
}

type Tab = "katalog" | "link" | "eigen";
interface Pasted { name: string; bytes: Uint8Array; url: string }

const STEPS = ["Kunde", "Was?", "Details", "Prüfen"];

/** Geführter Dialog: Kunde → Inhalt (Katalog, Link, eigene Idee) → Details → Prüfen */
export function NewOrderWizard({ dataDir, paths, dropRef, onClose, onCreated, fail, customerId: initialCustomer }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const [step, setStep] = useState(initialCustomer ? 1 : 0);
  const [people, setPeople] = useState<Person[]>([]);
  const [customerId, setCustomerId] = useState<string | null>(initialCustomer ?? null);
  const [tab, setTab] = useState<Tab>(paths.length ? "eigen" : "katalog");
  const [picks, setPicks] = useState<CatalogPick[]>([]);
  const [links, setLinks] = useState<LinkChoice[]>([]);
  const [linkUrl, setLinkUrl] = useState("");
  const [linkKey, setLinkKey] = useState(0);
  const [description, setDescription] = useState("");
  const [files, setFiles] = useState<string[]>(paths);
  const [pasted, setPasted] = useState<Pasted[]>([]);
  const [title, setTitle] = useState("");
  const [titleTouched, setTitleTouched] = useState(false);
  const [dueDate, setDueDate] = useState("");
  const [priority, setPriority] = useState(0);
  const [progress, setProgress] = useState<string | null>(null);
  const pricing = usePricing();

  useEffect(() => {
    ref.current?.showModal();
    ref.current?.querySelector<HTMLElement>("input")?.focus();
    api.people().then(setPeople).catch(fail);
  }, [fail]);

  // Weitere Dateien, die während des Assistenten ins Fenster gezogen werden
  useEffect(() => {
    dropRef.current = (p) => {
      setFiles((f) => [...f, ...p.filter((x) => !f.includes(x))]);
      setTab("eigen");
      // Der Kunde kommt immer zuerst; ohne Kunden bleibt der Assistent bei Schritt 1
      if (step === 0 && customerId) setStep(1);
    };
    return () => {
      dropRef.current = null;
    };
  }, [dropRef, step, customerId]);

  // Einfügen außerhalb von Eingabefeldern: Bilder sammeln, Links in den Link-Import
  useEffect(() => {
    const onPaste = async (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
      const dt = e.clipboardData;
      if (!dt) return;
      e.preventDefault();
      const imgs = Array.from(dt.files).filter((f) => f.type.startsWith("image/"));
      if (imgs.length) {
        const added = await Promise.all(
          imgs.map(async (f, i) => ({
            name: f.name && f.name !== "image.png" ? f.name : `eingefuegt-${Date.now()}-${i}.${f.type.split("/")[1] || "png"}`,
            bytes: new Uint8Array(await f.arrayBuffer()),
            url: URL.createObjectURL(f),
          })),
        );
        setPasted((p) => [...p, ...added]);
        setTab("eigen");
        return;
      }
      const text = dt.getData("text/plain").trim();
      if (isUrl(text)) {
        setTab("link");
        setLinkUrl(text);
        setLinkKey((k) => k + 1);
      } else if (text) {
        setDescription((d) => (d ? `${d}\n\n${text}` : text));
        setTab("eigen");
      }
    };
    document.addEventListener("paste", onPaste);
    return () => document.removeEventListener("paste", onPaste);
  }, []);

  // Titel-Vorschlag aus dem ersten Inhalt
  const suggestion = useMemo(() => {
    if (picks.length === 1 && !links.length) return picks[0].entry.title;
    if (links.length === 1 && !picks.length) return links[0].preview.title;
    if (picks.length + links.length > 1) return [...picks.map((p) => p.entry.title), ...links.map((l) => l.preview.title)].slice(0, 2).join(", ") + (picks.length + links.length > 2 ? " …" : "");
    const first = description.split("\n").find((l) => l.trim());
    if (first) return first.trim().slice(0, 60);
    if (files.length) return baseName(files[0]);
    return "";
  }, [picks, links, description, files]);

  useEffect(() => {
    if (!titleTouched) setTitle(suggestion);
  }, [suggestion, titleTouched]);

  const hasContent = picks.length + links.length + files.length + pasted.length > 0 || description.trim().length > 0;
  const canNext = [!!customerId, hasContent, title.trim().length > 0, !!customerId][step];

  async function create() {
    try {
      setProgress("Auftrag anlegen …");
      if (!customerId) throw new Error("Bitte einen Kunden wählen.");
      const o = await api.createOrder({ title, description, priority, dueDate: dueDate || null, personId: customerId });
      for (const p of picks) {
        setProgress(`Katalog: ${p.entry.title} …`);
        await api.catalogAddToOrder(o.id, p.entry.id, p.quantity, p.colorWish);
      }
      for (const l of links) {
        setProgress(`${l.preview.sourceLabel}: ${l.preview.title}${l.fileIds.length ? " (Dateien werden geladen)" : ""} …`);
        await api.linkImport(o.id, l.preview, l.fileIds);
      }
      if (files.length) {
        setProgress("Dateien kopieren …");
        await api.addPaths(o.id, files);
      }
      for (const p of pasted) await api.addBytes(o.id, p.name, p.bytes);
      onCreated(o.id);
    } catch (e) {
      fail(e);
      setProgress(null);
    }
  }

  async function chooseFiles() {
    const sel = await pickFiles({ multiple: true, title: "Dateien hinzufügen" });
    const p = Array.isArray(sel) ? sel : sel ? [sel] : [];
    setFiles((f) => [...f, ...p.filter((x) => !f.includes(x))]);
  }

  const customer = people.find((p) => p.id === customerId) ?? null;
  const contentSummary = [
    picks.length && `${picks.length}× Katalog`,
    links.length && `${links.length}× Link`,
    (files.length + pasted.length) && `${files.length + pasted.length} Datei(en)/Bild(er)`,
    description.trim() && "Beschreibung",
  ].filter(Boolean).join(" · ");

  return (
    <dialog ref={ref} className="dialog wizard" onCancel={(e) => { e.preventDefault(); if (!progress) onClose(); }}>
      <div className="dialog-head">
        <h2>Neuer Auftrag</h2>
        <button className="icon-btn close" onClick={onClose} disabled={!!progress} aria-label="Schließen"><X size={16} /></button>
      </div>
      <ol className="steps">
        {STEPS.map((s, i) => (
          <li key={s} className={i === step ? "on" : i < step ? "done" : ""}>
            <button className="ghost" disabled={i > step || !!progress} onClick={() => setStep(i)}>{i + 1}. {s}</button>
          </li>
        ))}
      </ol>

      <div className="wizard-body">
        {step === 0 && (
          <div className="stack">
            <CustomerPicker
              people={people}
              value={customerId}
              autoFocus
              fail={fail}
              onChange={(id, created) => {
                if (created) setPeople((l) => [...l.filter((p) => p.id !== id), created]);
                setCustomerId(id);
                setStep(1);
              }}
            />
          </div>
        )}

        {step === 1 && (
          <div className="stack">
            <div className="tabs" role="tablist">
              {([["katalog", "Aus dem Katalog"], ["link", "Von einem Link"], ["eigen", "Eigene Idee / Dateien"]] as [Tab, string][]).map(([t, l]) => (
                <button key={t} role="tab" aria-selected={tab === t} className={tab === t ? "on" : ""} onClick={() => setTab(t)}>{l}</button>
              ))}
            </div>
            {tab === "katalog" && <CatalogPicker dataDir={dataDir} picks={picks} onChange={setPicks} fail={fail} />}
            {tab === "link" && (
              <>
                {links.length > 0 && (
                  <ul className="mini-list">
                    {links.map((l, i) => (
                      <li key={i}>
                        <span className="grow">{l.preview.sourceLabel}: {l.preview.title}{l.fileIds.length ? ` · ${l.fileIds.length} Datei(en)` : ""}</span>
                        <button className="ghost icon-btn" onClick={() => setLinks(links.filter((_, j) => j !== i))} aria-label="Entfernen"><X size={14} /></button>
                      </li>
                    ))}
                  </ul>
                )}
                <LinkImport key={linkKey} initialUrl={linkUrl} action="Zum Auftrag hinzufügen" fail={fail}
                  onConfirm={(c) => { setLinks((ls) => [...ls, c]); setLinkUrl(""); setLinkKey((k) => k + 1); }} />
              </>
            )}
            {tab === "eigen" && (
              <>
                <label className="block">
                  Was genau wird gewünscht?
                  <textarea rows={5} value={description} onChange={(e) => setDescription(e.target.value)}
                    placeholder="Beschreibung, Maße, Farbe, Anlass … Nachrichten aus WhatsApp lassen sich hier einfügen." />
                </label>
                <div className="dropzone">
                  <span>Bilder und Dateien ins Fenster ziehen, mit ⌘V einfügen oder</span>
                  <button onClick={chooseFiles}>Datei wählen …</button>
                </div>
                {(files.length > 0 || pasted.length > 0) && (
                  <ul className="mini-list">
                    {files.map((f) => (
                      <li key={f}><span className="grow">{baseName(f, true)}</span>
                        <button className="ghost icon-btn" onClick={() => setFiles(files.filter((x) => x !== f))} aria-label="Entfernen"><X size={14} /></button></li>
                    ))}
                    {pasted.map((p, i) => (
                      <li key={p.url}><img src={p.url} alt="" className="mini-thumb" /><span className="grow">{p.name}</span>
                        <button className="ghost icon-btn" onClick={() => setPasted(pasted.filter((_, j) => j !== i))} aria-label="Entfernen"><X size={14} /></button></li>
                    ))}
                  </ul>
                )}
              </>
            )}
            <p className="hint">{contentSummary ? `Bisher: ${contentSummary}` : "Mindestens eines auswählen."}</p>
          </div>
        )}

        {step === 2 && (
          <div className="stack">
            <label className="block">
              Titel
              <input autoFocus value={title} onChange={(e) => { setTitle(e.target.value); setTitleTouched(true); }} />
            </label>
            {tab !== "eigen" && (
              <label className="block">
                Beschreibung / Notiz
                <textarea rows={3} value={description} onChange={(e) => setDescription(e.target.value)} placeholder="optional" />
              </label>
            )}
            <div className="grid2">
              <label>Fällig bis<input type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} /></label>
              <label>
                Priorität
                <select value={priority} onChange={(e) => setPriority(Number(e.target.value))}>
                  {PRIORITY.map((p, i) => <option key={p} value={i}>{p}</option>)}
                </select>
              </label>
            </div>
          </div>
        )}

        {step === 3 && (
          <div className="summary">
            <dl>
              <dt>Titel</dt><dd>{title}</dd>
              <dt>Kunde</dt><dd>{customer ? <>{customer.name} <span className="muted">{customerNo(customer.number)}</span></> : <span className="late">fehlt</span>}</dd>
              {dueDate && (<><dt>Fällig</dt><dd>{new Date(dueDate + "T00:00:00").toLocaleDateString("de-DE")}</dd></>)}
              {priority > 0 && (<><dt>Priorität</dt><dd>{PRIORITY[priority]}</dd></>)}
              {picks.length > 0 && (<><dt>Katalog</dt><dd>{picks.map((p) => `${p.quantity}× ${p.entry.title}${p.colorWish ? ` (${p.colorWish})` : ""}`).join(", ")}</dd></>)}
              {links.length > 0 && (<><dt>Links</dt><dd>{links.map((l) => `${l.preview.sourceLabel}: ${l.preview.title}${l.fileIds.length ? ` + ${l.fileIds.length} Datei(en)` : ""}`).join(", ")}</dd></>)}
              {(files.length + pasted.length) > 0 && (<><dt>Dateien</dt><dd>{[...files.map((f) => baseName(f, true)), ...pasted.map((p) => p.name)].join(", ")}</dd></>)}
              {description.trim() && (<><dt>Beschreibung</dt><dd className="pre">{description}</dd></>)}
            </dl>
            <CostBox usage={catalogUsage(picks.map((p) => ({ entry: p.entry, quantity: p.quantity })))} pricing={pricing} compact />
            {progress && <p className="progress">{progress}</p>}
          </div>
        )}
      </div>

      <footer className="row-end">
        {step > 0 && <button onClick={() => setStep(step - 1)} disabled={!!progress}>Zurück</button>}
        <span className="spacer" />
        <button onClick={onClose} disabled={!!progress}>Abbrechen</button>
        {step < 3 ? (
          <button className="primary" disabled={!canNext} onClick={() => setStep(step + 1)}>Weiter</button>
        ) : (
          <button className="primary" disabled={!!progress || !title.trim()} onClick={create}>Auftrag anlegen</button>
        )}
      </footer>
    </dialog>
  );
}

function baseName(p: string, keepExt = false) {
  const n = p.split(/[\\/]/).pop() ?? p;
  return keepExt ? n : n.replace(/\.[^.]+$/, "");
}
