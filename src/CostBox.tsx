import { useEffect, useState } from "react";
import { Calculator } from "lucide-react";
import { fmtDuration, fmtGrams } from "./api";
import { BASIS_LABEL, calc, eur, pricingApi, type Pricing, type Usage } from "./pricing";

let cached: Pricing | null = null;

/** Kalkulationswerte einmal laden (und nach dem Speichern in den Einstellungen neu). */
export function usePricing(rev = 0) {
  const [p, setP] = useState<Pricing | null>(cached);
  useEffect(() => {
    pricingApi.get().then((x) => { cached = x; setP(x); }).catch(() => {});
  }, [rev]);
  return p;
}

/** Kosten und Preisvorschlag; „tatsächlich“, sobald gedruckt wurde. */
export function CostBox({ usage, pricing, compact }: { usage: Usage | null; pricing: Pricing | null; compact?: boolean }) {
  const [open, setOpen] = useState(false);
  if (!pricing) return null;
  if (!usage) {
    return (
      <div className="cost-box empty">
        <Calculator size={14} />
        <span className="muted small">Noch keine Kalkulation: Projekt aus dem Katalog wählen oder in Bambu Studio slicen und im Auftragsordner speichern.</span>
      </div>
    );
  }
  const c = calc(pricing, usage);
  return (
    <div className={`cost-box ${usage.basis}`}>
      <div className="cost-main">
        <div>
          <div className="muted small">Verkaufspreis {usage.basis === "tatsaechlich" ? "" : "(Vorschlag)"}</div>
          <div className="cost-price">{eur(c.price)}</div>
        </div>
        <div>
          <div className="muted small">Kosten</div>
          <div className="cost-cost">{eur(c.cost)}</div>
        </div>
        <div className="cost-meta">
          <span className={`basis basis-${usage.basis}`}>{BASIS_LABEL[usage.basis]}</span>
          <span className="muted small">{fmtGrams(c.grams)} · {c.hours ? fmtDuration(c.hours * 3600) : "Druckzeit unbekannt"}</span>
        </div>
        {!compact && <button className="ghost small-btn" onClick={() => setOpen((x) => !x)}>{open ? "Weniger" : "Details"}</button>}
      </div>
      {(open || compact) && (
        <dl className="cost-detail">
          <dt>Material</dt><dd>{eur(c.material)} <span className="muted">({usage.filaments.map((f) => `${f.type === "default" ? "Filament" : f.type} ${fmtGrams(f.grams)}`).join(", ")})</span></dd>
          <dt>Strom</dt><dd>{eur(c.energy)}</dd>
          <dt>Maschine</dt><dd>{eur(c.machine)}</dd>
          {c.buffer > 0 && (<><dt>Fehldruck-Puffer</dt><dd>{eur(c.buffer)}</dd></>)}
          <dt>Pauschale</dt><dd>{eur(c.handling)}</dd>
        </dl>
      )}
    </div>
  );
}
