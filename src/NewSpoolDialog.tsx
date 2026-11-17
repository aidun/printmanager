import { useMemo, useState } from "react";
import { api, type Spool, type WriteResult } from "./api";
import { Modal } from "./Modal";

/** Neue Spule(n) im Bambu-Konto anlegen. Marken und Materialien werden aus den vorhandenen Spulen vorgeschlagen. */
export function NewSpoolDialog({ spools, onClose, notify }: { spools: Spool[]; onClose: () => void; notify: (t: string, error?: boolean) => void }) {
  const [brand, setBrand] = useState("Generic");
  const [material, setMaterial] = useState("PLA");
  const [series, setSeries] = useState("");
  const [color, setColor] = useState("#ffffff");
  const [name, setName] = useState("");
  const [total, setTotal] = useState(1000);
  const [rest, setRest] = useState<number | null>(null);
  const [count, setCount] = useState(1);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<WriteResult | null>(null);

  const uniq = (xs: string[]) => [...new Set(xs.filter(Boolean))].sort((a, b) => a.localeCompare(b));
  const brands = useMemo(() => uniq(spools.map((s) => s.brand)), [spools]);
  const materials = useMemo(() => uniq(spools.map((s) => s.material)), [spools]);
  const products = useMemo(() => uniq(spools.filter((s) => s.material.toLowerCase() === material.toLowerCase()).map((s) => s.series)), [spools, material]);

  const go = async () => {
    setBusy(true);
    try {
      const r = await api.spoolCreate({
        brand, material, series, color, name, note, count,
        totalGrams: total, remainingGrams: rest ?? total,
      });
      setResult(r);
      if (r.status === "ok") {
        notify(r.message);
        onClose();
      }
    } catch (e) {
      setResult({ status: "fehler", message: String(e), cloudGrams: null });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Spule anlegen" onClose={onClose}>
      <p className="hint">Legt die Spule im Bambu-Konto an. Sie erscheint danach auch in Bambu Studio und Handy.</p>
      <div className="form-grid">
        <label>Marke<input list="ns-brands" value={brand} onChange={(e) => setBrand(e.target.value)} /></label>
        <label>Material<input list="ns-materials" value={material} onChange={(e) => setMaterial(e.target.value)} /></label>
        <label>Produkt<input list="ns-products" value={series} placeholder={material} onChange={(e) => setSeries(e.target.value)} /></label>
        <label>Name in Handy<input value={name} placeholder="z.B. Rot matt" onChange={(e) => setName(e.target.value)} /></label>
        <label>Farbe
          <span className="color-row">
            <input type="color" value={color} onChange={(e) => setColor(e.target.value)} />
            <input value={color} onChange={(e) => setColor(e.target.value)} />
          </span>
        </label>
        <label>Anzahl<input type="number" min={1} max={10} value={count} onChange={(e) => setCount(Math.max(1, Math.min(10, Number(e.target.value) || 1)))} /></label>
        <label>Gesamtgewicht (g)<input type="number" min={1} value={total} onChange={(e) => setTotal(Number(e.target.value) || 0)} /></label>
        <label>Rest (g)<input type="number" min={0} value={rest ?? total} onChange={(e) => setRest(Number(e.target.value))} /></label>
        <label className="wide">Notiz<input value={note} onChange={(e) => setNote(e.target.value)} /></label>
      </div>
      <datalist id="ns-brands">{brands.map((b) => <option key={b} value={b} />)}</datalist>
      <datalist id="ns-materials">{materials.map((m) => <option key={m} value={m} />)}</datalist>
      <datalist id="ns-products">{products.map((p) => <option key={p} value={p} />)}</datalist>
      {result && result.status !== "ok" && <p className={`push-msg ${result.status}`}>{result.message}</p>}
      <footer className="row-end">
        <button onClick={onClose}>Abbrechen</button>
        <button className="primary" onClick={go} disabled={busy || !material.trim() || total <= 0}>
          {busy ? "Legt an …" : count > 1 ? `${count} Spulen anlegen` : "Im Konto anlegen"}
        </button>
      </footer>
    </Modal>
  );
}
