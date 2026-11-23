// Kosten und Verkaufspreis eines Auftrags. Werte aus „Einstellungen → Kalkulation“ (pricing.json).
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { parsePlates, parseRunFilaments, type CatalogEntry, type ItemEstimate, type OrderDetail, type RunFilament } from "./api";

export interface Pricing {
  filamentEurPerKg: Record<string, number>;
  powerWatts: number;
  powerEurPerKwh: number;
  machineEurPerHour: number;
  failurePct: number;
  handlingEur: number;
  markupFactor: number;
  minPriceEur: number;
  roundToEur: number;
}

export const pricingApi = {
  get: () => tauriInvoke<Pricing>("pricing_get"),
  save: (pricing: Pricing) => tauriInvoke<void>("pricing_save", { pricing }),
};

/** Woher die Mengen stammen: tatsächlich (Drucker/Abzug), Slicer (gesliced im Auftrag), Katalog (Katalogprojekte) */
export type Basis = "tatsaechlich" | "teilweise" | "slicer" | "vorab" | "makerworld" | "katalog";
export const BASIS_LABEL: Record<Basis, string> = {
  tatsaechlich: "tatsächlich",
  teilweise: "teils gedruckt, teils geplant",
  slicer: "geschätzt (Slicer)",
  vorab: "geschätzt (Slicer, vorab)",
  makerworld: "geschätzt (MakerWorld)",
  katalog: "geschätzt (Katalog)",
};

export interface Usage {
  basis: Basis;
  filaments: { type: string; grams: number }[];
  seconds: number;
}

export interface Calc {
  material: number;
  energy: number;
  machine: number;
  buffer: number;
  handling: number;
  cost: number;
  price: number;
  grams: number;
  hours: number;
}

export function filamentPrice(p: Pricing, type: string) {
  const t = type.trim().toUpperCase();
  const hit = Object.entries(p.filamentEurPerKg).find(([k]) => k.toUpperCase() === t);
  return hit ? hit[1] : p.filamentEurPerKg.default ?? 25;
}

/** Kosten = (Material + Strom + Maschine) × (1 + Puffer) + Pauschale; Preis = Kosten ohne Pauschale × Faktor + Pauschale, aufgerundet */
export function calc(p: Pricing, u: Usage): Calc {
  const hours = u.seconds / 3600;
  const grams = u.filaments.reduce((a, f) => a + f.grams, 0);
  const material = u.filaments.reduce((a, f) => a + (f.grams / 1000) * filamentPrice(p, f.type), 0);
  const energy = hours * (p.powerWatts / 1000) * p.powerEurPerKwh;
  const machine = hours * p.machineEurPerHour;
  // Fehldruck-Puffer nur auf geschätzte Werte – tatsächliche enthalten Fehldrucke schon
  const base = material + energy + machine;
  const buffer = u.basis === "tatsaechlich" ? 0 : base * (p.failurePct / 100);
  const handling = p.handlingEur;
  const cost = base + buffer + handling;
  const raw = (base + buffer) * p.markupFactor + handling;
  const step = p.roundToEur > 0 ? p.roundToEur : 0.01;
  const price = Math.max(p.minPriceEur, Math.ceil(raw / step - 1e-9) * step);
  return { material, energy, machine, buffer, handling, cost, price, grams, hours };
}

const sumFils = (list: RunFilament[], into: Map<string, number>, factor = 1) => {
  for (const f of list) into.set(f.type || "?", (into.get(f.type || "?") ?? 0) + f.grams * factor);
};
const toList = (m: Map<string, number>) => [...m.entries()].map(([type, grams]) => ({ type, grams }));

/**
 * Mengen eines Auftrags aus seinen Drucken: zurückgemeldete Drucke (Drucker, Abzug oder Ergebnis gesetzt) mit den
 * tatsächlichen Werten, offene mit den geplanten (je Datei nur der neueste gesliced Stand, und nur wenn die Datei
 * nicht schon gedruckt ist). Bei Dateien mit mehreren Platten zählt jede Platte für sich: gedruckt tatsächlich,
 * offen geplant, übersprungen gar nicht. Ohne Drucke: die Schätzungen je Teil.
 * Basis: alles gedruckt „tatsächlich“, gemischt „teilweise“, nichts gedruckt „Slicer“.
 */
export function orderUsage(d: OrderDetail, catalog: Map<string, CatalogEntry>, estimates: ItemEstimate[] = []): Usage | null {
  const m = new Map<string, number>();
  let seconds = 0;
  let printed = 0;
  let planned = 0;
  const isDone = (r: (typeof d.runs)[number]) => !!(r.printerStatus || r.deducted || r.result !== "offen");
  const doneFiles = new Set(d.runs.filter(isDone).map((r) => r.fileName));
  const seen = new Set<string>();
  for (const r of d.runs) {
    const plates = parsePlates(r.plateDetails);
    if (isDone(r)) {
      const actual = parseRunFilaments(r.actualFilaments || "[]");
      sumFils(actual.length ? actual : parseRunFilaments(r.filaments), m);
      seconds += r.actualSeconds ?? r.printSeconds ?? 0;
      printed++;
    } else if (plates.length > 1) {
      // Datei mit mehreren Platten, noch nicht alle durch
      if (seen.has(r.fileName)) continue;
      seen.add(r.fileName);
      for (const p of plates) {
        if (p.status === "uebersprungen") continue;
        if (p.status) {
          sumFils(p.actualFilaments.length ? p.actualFilaments : p.filaments, m);
          seconds += p.actualSeconds ?? p.seconds ?? 0;
          printed++;
        } else {
          sumFils(p.filaments, m);
          seconds += p.seconds ?? 0;
          planned++;
        }
      }
    } else {
      // je Datei nur der neueste gesliced Stand (Liste ist neueste zuerst), nicht wenn die Datei schon gedruckt ist
      if (seen.has(r.fileName) || doneFiles.has(r.fileName)) continue;
      seen.add(r.fileName);
      sumFils(parseRunFilaments(r.filaments), m);
      seconds += r.printSeconds ?? 0;
      planned++;
    }
  }
  if (printed + planned > 0 && (seconds > 0 || m.size)) {
    const basis: Basis = planned === 0 ? "tatsaechlich" : printed === 0 ? "slicer" : "teilweise";
    return { basis, filaments: toList(m), seconds };
  }
  return itemsUsage(d, catalog, estimates);
}

/**
 * Vorab je Teil: Schätzung aus dem Slicen im Hintergrund, sonst aus dem MakerWorld-Profil, sonst aus dem Katalog.
 * Die Basis ist die unsicherste der verwendeten Quellen.
 */
export function itemsUsage(d: OrderDetail, catalog: Map<string, CatalogEntry>, estimates: ItemEstimate[]): Usage | null {
  const m = new Map<string, number>();
  let seconds = 0;
  const used = new Set<Basis>();
  for (const it of d.items) {
    const e = estimates.find((x) => x.itemId === it.id && x.basis);
    if (e) {
      sumFils(parseRunFilaments(e.filaments).filter((f) => f.grams > 0), m, it.quantity);
      seconds += (e.seconds ?? 0) * it.quantity;
      used.add(e.basis === "slicer" ? "vorab" : "makerworld");
    } else if (it.catalogId) {
      const u = catalogUsage([{ entry: catalog.get(it.catalogId), quantity: it.quantity }]);
      if (u) {
        for (const f of u.filaments) m.set(f.type, (m.get(f.type) ?? 0) + f.grams);
        seconds += u.seconds;
        used.add("katalog");
      }
    }
  }
  if (!m.size && !seconds) return null;
  const order: Basis[] = ["katalog", "makerworld", "vorab"];
  return { basis: order.find((b) => used.has(b)) ?? "katalog", filaments: toList(m), seconds };
}

/** Mengen aus Katalogprojekten (auch im Assistenten, bevor der Auftrag existiert). */
export function catalogUsage(parts: { entry: CatalogEntry | undefined; quantity: number }[]): Usage | null {
  const m = new Map<string, number>();
  let seconds = 0;
  for (const { entry, quantity } of parts) {
    if (!entry || (!entry.weightGrams && !entry.printSeconds)) continue;
    const fils = parseRunFilaments(entry.filaments).filter((f) => f.grams > 0);
    if (fils.length) sumFils(fils, m, quantity);
    else if (entry.weightGrams) m.set("default", (m.get("default") ?? 0) + entry.weightGrams * quantity);
    seconds += (entry.printSeconds ?? 0) * quantity;
  }
  return m.size || seconds ? { basis: "katalog", filaments: toList(m), seconds } : null;
}

export const eur = (x: number) => x.toLocaleString("de-DE", { style: "currency", currency: "EUR" });
