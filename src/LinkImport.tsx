import { useEffect, useState } from "react";
import { api, fmtDuration, fmtSize, isModelFile, type LinkPreview } from "./api";

/** Auswahl, die beim Import mitgenommen wird */
export interface LinkChoice {
  preview: LinkPreview;
  fileIds: string[];
}

interface Props {
  /** Link vorbelegen (z.B. aus der Zwischenablage) */
  initialUrl?: string;
  /** Text des Bestätigungsknopfs */
  action: string;
  /** nur Druckdateien anbieten (für den Katalog) */
  printFilesOnly?: boolean;
  busy?: boolean;
  onConfirm: (choice: LinkChoice) => void;
  onCancel?: () => void;
  fail: (e: unknown) => void;
}

/** Link einfügen → Vorschau von Printables, MakerWorld & Co. → Dateien auswählen */
export function LinkImport({ initialUrl = "", action, printFilesOnly, busy, onConfirm, onCancel, fail }: Props) {
  const [url, setUrl] = useState(initialUrl);
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<LinkPreview | null>(null);
  const [chosen, setChosen] = useState<string[]>([]);

  async function load(u = url) {
    if (!u.trim()) return;
    setLoading(true);
    setPreview(null);
    try {
      const p = await api.linkPreview(u.trim());
      setPreview(p);
      // Vorauswahl: 3MF bevorzugt, sonst alle STL; G-Code für andere Drucker nie
      const printable = p.files.filter((f) => isModelFile(f.name));
      const threeMf = printable.filter((f) => /\.3mf$/i.test(f.name));
      setChosen((threeMf.length ? threeMf : printable).map((f) => f.id));
    } catch (e) {
      fail(e);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (initialUrl) load(initialUrl);
  }, [initialUrl]); // eslint-disable-line react-hooks/exhaustive-deps

  const files = preview ? preview.files.filter((f) => !printFilesOnly || isModelFile(f.name)) : [];
  const toggle = (id: string) => setChosen((c) => (c.includes(id) ? c.filter((x) => x !== id) : [...c, id]));

  return (
    <div className="link-import">
      <div className="add-row">
        <input
          className="grow"
          autoFocus
          placeholder="Link von Printables, MakerWorld, Thingiverse …"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              load();
            }
          }}
        />
        <button onClick={() => load()} disabled={!url.trim() || loading}>{loading ? "Lädt …" : "Abrufen"}</button>
      </div>

      {preview && (
        <div className="preview">
          {preview.imageUrl && <img src={preview.imageUrl} alt="" referrerPolicy="no-referrer" />}
          <div className="preview-body">
            <div className="muted small">{preview.sourceLabel}</div>
            <div className="preview-title">{preview.title}</div>
            {(preview.designer || preview.license) && (
              <div className="muted small">
                {preview.designer && <>von {preview.designer}</>}
                {preview.designer && preview.license && " · "}
                {preview.license && <>Lizenz {preview.license}</>}
              </div>
            )}
            {preview.description && <p className="preview-desc">{preview.description.slice(0, 280)}{preview.description.length > 280 ? " …" : ""}</p>}
          </div>
        </div>
      )}

      {preview && preview.profiles.length > 0 && (
        <details className="profiles">
          <summary>{preview.profiles.length} Druckprofile auf {preview.sourceLabel}</summary>
          <ul>
            {preview.profiles.map((p, i) => (
              <li key={i}>
                {p.title} <span className="muted small">{fmtDuration(p.printSeconds)}{p.weightGrams ? ` · ${p.weightGrams} g` : ""}</span>
              </li>
            ))}
          </ul>
        </details>
      )}

      {files.length > 0 && (
        <div className="files">
          <div className="muted small">Dateien herunterladen:</div>
          {files.map((f) => (
            <label key={f.id} className="check">
              <input type="checkbox" checked={chosen.includes(f.id)} onChange={() => toggle(f.id)} />
              <span className="grow">{f.name}</span>
              <span className="muted small">{fmtSize(f.size)}</span>
            </label>
          ))}
        </div>
      )}

      {preview?.note && <p className="hint">{preview.note}</p>}

      {preview && (
        <footer className="row-end">
          {onCancel && <button onClick={onCancel}>Abbrechen</button>}
          <button
            className="primary"
            disabled={busy || (printFilesOnly && chosen.length === 0)}
            onClick={() => onConfirm({ preview, fileIds: chosen.filter((id) => files.some((f) => f.id === id)) })}
          >
            {busy ? "Lädt …" : action}
          </button>
        </footer>
      )}
    </div>
  );
}
