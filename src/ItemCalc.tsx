import { useEffect, useMemo, useState } from "react";
import { Calculator, Plus, RefreshCw, Save, X } from "lucide-react";
import {
  api, fmtDuration, fmtGrams, parseColorWish, parseRunFilaments, presetFor, presetLabel, rankSpools, spoolLabel,
  type CatalogEntry, type Item, type ItemEstimate, type ItemSetup, type ModelLayout, type ProfileChoice, type MwProfile, type ProfileCatalog, type SetupObject,
  type SetupSlot, type SlicePreset, type Spool,
} from "./api";
import { Modal } from "./Modal";

const PLATES = ["Textured PEI Plate", "Cool Plate", "Engineering Plate", "High Temp Plate", "Supertack Plate"];
const LAST = "pm.lastSliceProfile";
/** zuletzt aus dem Bambu-Konto bekannte eigene Druckermodelle (die Liste ist kurz nach dem Start noch leer) */
const OWN = "pm.ownPrinterModels";

/** Druckermodell aus dem Konto („X1 Carbon“, „A1 mini“, „H2C“) zur Kennung der Profilquelle */
const modelKey = (s: string) => s.toLowerCase().replace(/^bambu lab\s*/, "").replace(/[^a-z0-9]/g, "");
const readOwn = (): string[] => {
  try {
    return JSON.parse(localStorage.getItem(OWN) ?? "[]");
  } catch {
    return [];
  }
};
const LEVEL: Record<string, string> = { quality: "Qualität", normal: "Normal", fast: "Schnell" };
/** Farbabstand, bis zu dem eine Spule automatisch vorgeschlagen wird */
const NEAR = 60;

/** Kurzinfo zur Schätzung eines Teils in der Teilezeile */
export function EstimateTag({ e }: { e: ItemEstimate | undefined }) {
  if (!e) return null;
  const ready = e.setupState === "bereit"
    ? <span className="ready-chip" title="Profil, Farben und Filamente sind festgelegt">Druck vorbereitet</span>
    : null;
  if (e.error) return <span className="est-tag">{ready}<span className="warn" title={e.error}>Kalkulation: Fehler</span></span>;
  if (!e.basis) return ready && <span className="est-tag">{ready}</span>;
  const grams = parseRunFilaments(e.filaments).reduce((s, f) => s + f.grams, 0);
  return (
    <span className="est-tag" title={e.basis === "slicer" ? "im Hintergrund gesliced" : "aus dem MakerWorld-Druckprofil"}>
      {ready}≈ {e.seconds ? fmtDuration(e.seconds) : "?"} · {grams ? fmtGrams(grams) : "?"} · {e.basis === "slicer" ? "Slicer" : "MakerWorld"}
    </span>
  );
}

const parse = <T,>(json: string | undefined): T[] => {
  try {
    return JSON.parse(json || "[]") as T[];
  } catch {
    return [];
  }
};

/** eigene Druckermodelle aus dem Konto holen und merken (sonst die zuletzt bekannten) */
export async function refreshOwnModels(): Promise<string[]> {
  try {
    const r = await api.printers();
    const models = [...new Set(r.printers.map((p) => p.model).filter(Boolean))];
    if (models.length) {
      try {
        localStorage.setItem(OWN, JSON.stringify(models));
      } catch {
        /* egal */
      }
      return models;
    }
  } catch {
    /* ohne Konto: zuletzt bekannte */
  }
  return readOwn();
}

/** eigene Drucker aus der Profilquelle; sind keine bekannt, alle */
function ownPrinters(cat: ProfileCatalog, ownModels: string[]) {
  const own = cat.printers.filter((p) => ownModels.some((m) => modelKey(m) === p.id || modelKey(m) === modelKey(p.label)));
  return own.length ? own : cat.printers;
}

/**
 * Standardauswahl für ein Teil ohne gespeichertes Profil, wie sie der Dialog vorschlägt: eigener Drucker (der zuletzt
 * benutzte, sonst der erste), Düse 0.4, „Normal + Stützen“, Slots mit passender Spule (AMS zuerst) und Profil.
 * Ein gespeichertes Profil des Teils bleibt; dann kommt `null` zurück.
 */
export function defaultChoice(
  item: Item, setup: ItemSetup, cat: ProfileCatalog, spools: Spool[], ownModels: string[] = readOwn(),
): ProfileChoice | null {
  if (setup.estimate.process) return null;
  let last: Partial<ProfileChoice> = {};
  try {
    last = JSON.parse(localStorage.getItem(LAST) ?? "{}");
  } catch {
    /* egal */
  }
  const printers = ownPrinters(cat, ownModels);
  const printer = (printers.find((p) => p.id === last.printer) ?? printers[0])?.id ?? "";
  const nozzles = printers.find((p) => p.id === printer)?.nozzles ?? [];
  const nozzle = nozzles.includes(last.nozzle ?? "") ? last.nozzle! : nozzles.includes("0.4") ? "0.4" : nozzles[0] ?? "0.4";
  const fit = (kind: "process" | "filament") => cat.presets.filter((p) => p.kind === kind && p.printer === printer && p.nozzle === nozzle);
  const processes = fit("process");
  const process = (processes.find((p) => p.level === "normal" && p.support) ?? processes.find((p) => p.level === "normal") ?? processes[0])?.file ?? "";
  const filaments = fit("filament");
  const slots = initialSlots(item, parse<SetupSlot>(setup.estimate.slots), setup.layout, spools).map((s) => {
    const sp = spools.find((x) => x.id === s.spoolId);
    const m = setup.layout?.slots.find((x) => x.slot === s.slot);
    const preset = (sp ? presetFor(filaments, sp)?.preset : undefined)
      ?? presetFor(filaments, { material: s.material, name: m?.name })?.preset
      ?? filaments.find((p) => p.file === last.filament);
    return { ...s, preset: preset?.file ?? "" };
  });
  const saved = parse<SetupObject>(setup.estimate.objects);
  const fromModel = (setup.layout?.objects ?? []).map(({ index, name, slot }) => ({ index, name, slot }));
  return {
    source: cat.source, printer, nozzle, process, bedType: last.bedType || PLATES[0],
    filament: slots.find((s) => s.preset)?.preset ?? "",
    slots, objects: saved.length && saved.length === fromModel.length ? saved : fromModel,
  };
}

/** Spulen im AMS zuerst, sonst Reihenfolge wie gerankt */
function amsFirst<T extends { spool: Spool }>(list: T[]): T[] {
  return [...list.filter((x) => x.spool.inPrinter > 0), ...list.filter((x) => !x.spool.inPrinter)];
}

/** Startwerte für Slots: gespeichert, sonst aus dem Modell und dem Farbwunsch, mit passender Spule und Preset */
function initialSlots(item: Item, saved: SetupSlot[], layout: ModelLayout | null, spools: Spool[]): SetupSlot[] {
  if (saved.length) return saved;
  const wish = parseColorWish(item.colorWish);
  const model = layout?.slots.length ? layout.slots : [{ slot: 1, material: wish.material ?? "PLA", color: wish.color ?? "", name: "" }];
  // benutzt: Slots der Objekte und der Teile mit eigener Farbe
  const used = new Set(layout?.objects.flatMap((o) => [o.slot, ...(o.fixed ? o.parts ?? [] : [])]) ?? [1]);
  // nur benutzte Slots übernehmen; bei bemalten Modellen alle (die Farben der Flächen stehen im Modell)
  const list = model.filter((m) => used.has(m.slot) || model.length === 1 || layout?.painted);
  const taken = new Set<string>();
  return list.map((m) => {
    // Farbwunsch gilt, wenn das Modell nur eine Farbe hat
    const color = (list.length === 1 && wish.color) || m.color || "";
    const material = (list.length === 1 && wish.material) || m.material || "PLA";
    const near = amsFirst(rankSpools(spools, { color: color || null, material }).filter((x) => !color || x.distance < NEAR));
    // eine Spule pro Slot bevorzugen; gibt es keine andere passende, dieselbe (Hinweis „Zusammenlegen“ im Dialog)
    const best = near.find((x) => !taken.has(x.spool.id)) ?? near[0];
    if (best) taken.add(best.spool.id);
    return { slot: m.slot, preset: "", material, color: best?.spool.color.slice(0, 7).toUpperCase() || color, spoolId: best?.spool.id ?? null };
  });
}

/**
 * Kalkulation und Druckvorbereitung für ein Teil: Drucker, Düse, Prozess und Platte wählen, Farben und Filamente
 * je Slot festlegen (mit Spule aus dem Bestand), Objekte den Slots zuordnen; dann im Hintergrund slicen
 * oder bei MakerWorld-Projekten ein Druckprofil von MakerWorld übernehmen.
 */
export function ItemCalcDialog({
  item, estimate, entry, spools, orderBasis, onClose, onChanged, fail,
}: {
  item: Item; estimate: ItemEstimate | undefined; entry: CatalogEntry | undefined; spools: Spool[];
  /** Basis der Auftragskalkulation; tatsächlich oder Slicer heißt: genauere Werte liegen schon vor */
  orderBasis?: string;
  onClose: () => void; onChanged: (e: ItemEstimate) => void; fail: (e: unknown) => void;
}) {
  const [cat, setCat] = useState<ProfileCatalog | null>(null);
  const [catErr, setCatErr] = useState<string | null>(null);
  const last = (() => {
    try {
      return JSON.parse(localStorage.getItem(LAST) ?? "{}");
    } catch {
      return {};
    }
  })();
  const init = estimate?.process ? estimate : last;
  const [printer, setPrinter] = useState<string>(init.printer ?? "");
  const [nozzle, setNozzle] = useState<string>(init.nozzle ?? "0.4");
  const [process, setProcess] = useState<string>(estimate?.process ?? "");
  const [plate, setPlate] = useState<string>(init.bedType || PLATES[0]);
  const [layout, setLayout] = useState<ModelLayout | null>(null);
  const [layoutErr, setLayoutErr] = useState<string | null>(null);
  const [slots, setSlots] = useState<SetupSlot[] | null>(null);
  const [objects, setObjects] = useState<SetupObject[]>([]);
  /** gewählte Platten; leer = alle */
  const [plates, setPlates] = useState<number[]>(() => parse<number>(estimate?.plates));
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(estimate?.error || null);
  const [mw, setMw] = useState<MwProfile[] | null>(null);
  const [ownModels, setOwnModels] = useState<string[]>(readOwn);
  const isMw = entry?.origin === "makerworld";

  const loadCat = (refresh = false) =>
    api.sliceProfiles(refresh).then((c) => { setCat(c); setCatErr(null); }).catch((e) => setCatErr(String(e)));
  useEffect(() => {
    loadCat();
    api.printers()
      .then((r) => {
        const models = [...new Set(r.printers.map((p) => p.model).filter(Boolean))];
        if (!models.length) return;
        setOwnModels(models);
        try {
          localStorage.setItem(OWN, JSON.stringify(models));
        } catch {
          /* egal */
        }
      })
      .catch(() => {});
    if (isMw) api.itemMakerworldProfiles(item.id).then(setMw).catch(() => setMw([]));
    api.itemSetup(item.id)
      .then((s) => {
        setLayout(s.layout);
        setLayoutErr(s.layoutError);
        setSlots(initialSlots(item, parse<SetupSlot>(s.estimate.slots), s.layout, spools));
        const saved = parse<SetupObject>(s.estimate.objects);
        const fromModel = (s.layout?.objects ?? []).map(({ index, name, slot }) => ({ index, name, slot }));
        setObjects(saved.length && saved.length === fromModel.length ? saved : fromModel);
      })
      .catch((e) => { setLayoutErr(String(e)); setSlots(initialSlots(item, [], null, spools)); });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const allPrinters = cat?.printers ?? [];
  // nur eigene Drucker zur Wahl; sind keine bekannt (nicht angemeldet), alle aus der Profilquelle
  const printers = cat ? ownPrinters(cat, ownModels) : allPrinters;
  const nozzles = printers.find((p) => p.id === printer)?.nozzles ?? [];
  const fit = (kind: "process" | "filament") => (cat?.presets ?? []).filter((p) => p.kind === kind && p.printer === printer && p.nozzle === nozzle);
  const processes = useMemo(() => fit("process"), [cat, printer, nozzle]); // eslint-disable-line react-hooks/exhaustive-deps
  const filaments = useMemo(() => fit("filament").sort((a, b) => a.name.localeCompare(b.name)), [cat, printer, nozzle]); // eslint-disable-line react-hooks/exhaustive-deps
  const allPresets = cat?.presets ?? [];

  // Auswahl gültig halten, wenn Drucker oder Düse wechseln
  useEffect(() => {
    if (!cat) return;
    // immer ein vorhandener Drucker: der zuletzt benutzte, wenn es ihn gibt, sonst der erste eigene
    if (printers.length && !printers.some((p) => p.id === printer)) {
      setPrinter((printers.find((p) => p.id === last.printer) ?? printers[0]).id);
    }
    if (printer && nozzles.length && !nozzles.includes(nozzle)) setNozzle(nozzles.includes("0.4") ? "0.4" : nozzles[0]);
  }, [cat, printer, nozzle, ownModels]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (processes.length && !processes.some((p) => p.file === process)) {
      // gleiche Stufe wie bisher (anderer Drucker oder Düse), sonst Standard: Normal mit Stützen
      const old = (cat?.presets ?? []).find((p) => p.file === process);
      const pick = (old && processes.find((p) => p.level === old.level && !!p.support === !!old.support))
        ?? processes.find((p) => p.level === "normal" && p.support)
        ?? processes.find((p) => p.level === "normal")
        ?? processes[0];
      setProcess(pick.file);
    }
  }, [processes]); // eslint-disable-line react-hooks/exhaustive-deps
  // Filament-Presets der Slots zum Drucker passend halten: gleiches Filament am neuen Drucker, sonst Vorschlag
  useEffect(() => {
    if (!slots || !filaments.length) return;
    const next = slots.map((s) => {
      if (filaments.some((p) => p.file === s.preset)) return s;
      const old = allPresets.find((p) => p.file === s.preset);
      const same = old?.filament ? filaments.find((p) => p.filament === old.filament) : undefined;
      return { ...s, preset: (same ?? suggest(s))?.file ?? "" };
    });
    if (next.some((s, i) => s.preset !== slots[i].preset)) setSlots(next);
  }, [filaments, slots]); // eslint-disable-line react-hooks/exhaustive-deps

  const spoolOf = (id: string | null) => spools.find((s) => s.id === id);
  /** Slots, die Teile mit eigener Farbe im Modell belegen; die ändert nur Bambu Studio */
  const partSlots = new Map<number, string[]>();
  for (const o of layout?.objects ?? []) {
    if (o.fixed) for (const p of o.parts ?? []) partSlots.set(p, [...(partSlots.get(p) ?? []), o.name]);
  }
  const isUsed = (n: number) => !objects.length || objects.some((o) => o.slot === n) || partSlots.has(n);
  /** Preset-Vorschlag für einen Slot: aus der Spule, sonst aus dem Modell, sonst das zuletzt benutzte Filament */
  function suggest(s: SetupSlot): SlicePreset | undefined {
    const sp = spoolOf(s.spoolId);
    if (sp) return presetFor(filaments, sp)?.preset;
    const m = layout?.slots.find((x) => x.slot === s.slot);
    return presetFor(filaments, { material: s.material, name: m?.name })?.preset
      ?? filaments.find((p) => p.file === last.filament) ?? undefined;
  }
  const match = (s: SetupSlot) => {
    const sp = spoolOf(s.spoolId);
    if (!sp) return null;
    const best = presetFor(filaments, sp);
    return best && best.preset.file === s.preset && !best.exact ? `Kein eigenes Profil für ${spoolLabel(sp)}; ähnliches Profil gewählt.` : null;
  };

  const setSlot = (slot: number, patch: Partial<SetupSlot>) =>
    setSlots((l) => (l ?? []).map((s) => (s.slot === slot ? { ...s, ...patch } : s)));
  const pickSpool = (s: SetupSlot, id: string) => {
    const sp = spoolOf(id || null);
    if (!sp) return setSlot(s.slot, { spoolId: null });
    setSlot(s.slot, {
      spoolId: sp.id, color: sp.color.slice(0, 7).toUpperCase(), material: sp.material,
      preset: presetFor(filaments, sp)?.preset.file ?? s.preset,
    });
  };
  const addSlot = () => setSlots((l) => {
    const list = l ?? [];
    const n = Math.max(0, ...list.map((s) => s.slot)) + 1;
    return [...list, { slot: n, preset: list[0]?.preset ?? "", material: list[0]?.material ?? "PLA", color: "", spoolId: null }];
  });
  const removeSlot = (n: number) => setSlots((l) => (l ?? []).filter((s) => s.slot !== n));
  /** Slot `from` in Slot `to` aufgehen lassen: Objekte wandern mit, der Slot entfällt */
  const mergeSlot = (from: number, to: number) => {
    setObjects((l) => l.map((o) => (o.slot === from ? { ...o, slot: to } : o)));
    removeSlot(from);
  };

  // Mengen und Warnungen
  const usage = parseRunFilaments(estimate?.filaments ?? "[]");
  /** Verbrauch eines Slots: nach Slot-Nummer, bei älteren Schätzungen ohne Slot nach Spule bzw. Farbe */
  const slotGrams = (s: SetupSlot) =>
    usage.filter((f) => (f.slot != null ? f.slot === s.slot : f.spoolId ? f.spoolId === s.spoolId : f.color === s.color)).reduce((a, f) => a + f.grams, 0);
  /** früherer Slot mit derselben Spule bzw. ohne Spule mit gleichem Profil und gleicher Farbe */
  const twinOf = (s: SetupSlot) => slots?.find((o) => o.slot < s.slot && (s.spoolId
    ? o.spoolId === s.spoolId
    : !o.spoolId && !!s.preset && o.preset === s.preset && o.color.toUpperCase() === s.color.toUpperCase()));
  const warnings = (s: SetupSlot): string[] => {
    const out: string[] = [];
    const sp = spoolOf(s.spoolId);
    const preset = allPresets.find((p) => p.file === s.preset);
    if (sp && preset && !presetLabel(preset).toLowerCase().includes(sp.material.toLowerCase().split(/[\s+-]/)[0])) {
      out.push(`Profil „${presetLabel(preset)}“ passt nicht zum Material ${sp.material}.`);
    }
    // gleiche Spule in mehreren Slots: Bedarf zusammenzählen und nur beim ersten Slot melden
    const sameSpool = sp ? (slots ?? []).filter((o) => o.spoolId === sp.id) : [];
    const need = (sp ? sameSpool.reduce((a, o) => a + slotGrams(o), 0) : slotGrams(s)) * item.quantity;
    if (sp && need > 0 && sp.remainingGrams < need && sameSpool[0]?.slot === s.slot) {
      out.push(`Spule reicht nicht: ${fmtGrams(need)} nötig, ${fmtGrams(sp.remainingGrams)} übrig.`);
    }
    const m = match(s);
    if (m) out.push(m);
    return out;
  };
  const usedSlots = slots?.filter((s) => isUsed(s.slot)) ?? [];

  const choice = () => ({
    source: cat?.source ?? "", printer, nozzle, process, bedType: plate,
    filament: slots?.find((s) => s.preset)?.preset ?? "",
    slots: slots ?? undefined, objects,
    // alle gewählt = leer (neue Platten zählen dann automatisch mit)
    plates: plates.length >= (layout?.plates?.length ?? 0) ? [] : plates,
  });
  const save = async () => {
    const c = choice();
    try {
      localStorage.setItem(LAST, JSON.stringify({ source: c.source, printer, nozzle, process, filament: c.filament, bedType: plate }));
    } catch {
      /* egal */
    }
    return api.itemSetProfile(item.id, c);
  };

  const slice = async () => {
    setBusy(true);
    setMsg(null);
    try {
      await save();
      onChanged(await api.itemSlice(item.id));
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const takeMw = async (p: MwProfile) => {
    try {
      onChanged(await api.itemEstimateMakerworld(item.id, p.id));
    } catch (e) {
      fail(e);
    }
  };

  const incomplete = !process || !slots?.length || usedSlots.some((s) => !s.preset);

  return (
    <Modal title={`Kalkulation und Druck: ${item.title}`} onClose={onClose} wide>
      <p className="hint">
        Lege Drucker, Profil, Farben und Filamente fest. Daraus schätzt die App Druckzeit und Material, bevor du selbst slicest.
        Liegt später eine gesliced Datei im Auftragsordner, gelten deren Werte, nach dem Druck die tatsächlichen.
      </p>
      {(orderBasis === "tatsaechlich" || orderBasis === "slicer") && (
        <p className="calc-known">
          {orderBasis === "tatsaechlich"
            ? "Dieser Auftrag ist schon gedruckt. Die Kalkulation nutzt die tatsächlichen Werte des Drucks; Slicen ist nicht nötig."
            : "Im Auftragsordner liegt schon eine gesliced Datei. Die Kalkulation nutzt deren Werte; Slicen ist nur zum Vergleich nötig."}
        </p>
      )}
      {estimate?.basis && <p className="small">Aktuell: <EstimateTag e={estimate} /></p>}

      {isMw && (
        <div className="calc-block">
          <h4>MakerWorld-Druckprofil</h4>
          {!mw && <p className="muted small">Profile werden geladen …</p>}
          {mw && mw.length === 0 && <p className="muted small">Keine Druckprofile gefunden. Ist das Bambu-Konto verbunden?</p>}
          <ul className="mw-profiles">
            {mw?.map((p) => (
              <li key={p.id}>
                <button className={`cust-opt ${estimate?.mwInstance === p.id ? "on" : ""}`} onClick={() => takeMw(p)} disabled={!p.seconds && !p.grams}>
                  <span className="grow">{p.title || "Profil"}{p.default ? " · Standard" : ""}</span>
                  <span className="muted small">{p.seconds ? fmtDuration(p.seconds) : "–"} · {p.grams ? fmtGrams(p.grams) : "–"}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="calc-block">
        <h4>Drucker und Profil</h4>
        {catErr && <p className="warn small">{catErr}</p>}
        <div className="form-grid four">
          <label>Drucker
            <select value={printer} onChange={(e) => setPrinter(e.target.value)}>
              {printers.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
            </select>
          </label>
          <label>Düse
            <select value={nozzle} onChange={(e) => setNozzle(e.target.value)}>
              {nozzles.map((n) => <option key={n} value={n}>{n} mm</option>)}
            </select>
          </label>
          <label>Prozess
            <select value={process} onChange={(e) => setProcess(e.target.value)}>
              {processes.map((p) => (
                <option key={p.file} value={p.file}>{LEVEL[p.level ?? ""] ?? p.name}{p.support ? " + Stützen" : ""}{p.layerHeight ? ` · ${p.layerHeight} mm` : ""}</option>
              ))}
            </select>
          </label>
          <label>Druckplatte
            <select value={plate} onChange={(e) => setPlate(e.target.value)}>
              {PLATES.map((p) => <option key={p} value={p}>{p}</option>)}
            </select>
          </label>
        </div>
      </div>

      {(layout?.plates?.length ?? 0) > 1 && (
        <div className="calc-block">
          <h4>Platten</h4>
          <p className="muted small">Gerechnet wird nur mit den Platten, die du druckst.</p>
          <div className="plate-picks">
            {layout!.plates!.map((p) => {
              const on = plates.length === 0 || plates.includes(p.index);
              const all = layout!.plates!.map((x) => x.index);
              return (
                <label key={p.index} className="check">
                  <input type="checkbox" checked={on} onChange={() => {
                    const cur = plates.length === 0 ? all : plates;
                    const next = on ? cur.filter((x) => x !== p.index) : [...cur, p.index].sort((a, b) => a - b);
                    setPlates(next.length === 0 ? cur : next.length === all.length ? [] : next);
                  }} />
                  Platte {p.index}{p.name ? ` · ${p.name}` : ""}
                </label>
              );
            })}
          </div>
        </div>
      )}

      <div className="calc-block">
        <h4>Farben und Filamente</h4>
        {!slots && <p className="muted small">Modell wird gelesen …</p>}
        {layoutErr && <p className="muted small">{layoutErr} Ohne Modelldatei gilt ein Filament für das ganze Teil.</p>}
        {layout?.painted && <p className="muted small">Das Modell ist bemalt. Die Farben der bemalten Flächen kommen aus dem Modell; hier legst du fest, welches Filament in welchem Slot liegt.</p>}
        {slots && (
          <ul className="setup-slots">
            {slots.map((s) => {
              const sp = spoolOf(s.spoolId);
              const ranked = amsFirst(rankSpools(spools, { color: s.color || null, material: null }));
              const w = warnings(s);
              const twin = twinOf(s);
              const unused = !isUsed(s.slot);
              const byParts = partSlots.get(s.slot);
              return (
                <li key={s.slot} className={unused ? "unused" : ""}>
                  <span className="slot-no" title={`Slot ${s.slot}`}>{s.slot}</span>
                  <input type="color" className="slot-color" value={s.color || "#cccccc"} aria-label={`Farbe Slot ${s.slot}`}
                    onChange={(e) => setSlot(s.slot, { color: e.target.value.toUpperCase(), spoolId: null })} />
                  <select className="grow" value={s.spoolId ?? ""} onChange={(e) => pickSpool(s, e.target.value)} aria-label={`Spule Slot ${s.slot}`}>
                    <option value="">Ohne Spule ({s.material || "Material offen"})</option>
                    {[["Im AMS", ranked.filter((r) => r.spool.inPrinter > 0)], ["Weitere Spulen", ranked.filter((r) => !r.spool.inPrinter)]].map(([label, list]) =>
                      (list as typeof ranked).length > 0 && (
                        <optgroup key={label as string} label={label as string}>
                          {(list as typeof ranked).map(({ spool: x }) => (
                            <option key={x.id} value={x.id}>{x.amsSlot ? `${x.amsSlot} · ` : ""}{spoolLabel(x).includes(x.material) ? "" : `${x.material} `}{spoolLabel(x)} · {fmtGrams(x.remainingGrams)}</option>
                          ))}
                        </optgroup>
                      ))}
                  </select>
                  <select className="grow" value={s.preset} onChange={(e) => setSlot(s.slot, { preset: e.target.value })} aria-label={`Filament-Profil Slot ${s.slot}`}>
                    {!s.preset && <option value="">Filament-Profil wählen</option>}
                    {filaments.map((p) => <option key={p.file} value={p.file}>{presetLabel(p)}</option>)}
                  </select>
                  {slots.length > 1 && unused
                    ? <button className="icon-btn" onClick={() => removeSlot(s.slot)} title="Slot entfernen" aria-label="Slot entfernen"><X size={13} /></button>
                    : <span className="icon-gap" />}
                  {unused && <p className="muted small slot-note">Keinem Objekt zugeordnet</p>}
                  {twin && (
                    <p className="muted small slot-note">
                      {s.spoolId ? `Gleiche Spule wie Slot ${twin.slot}.` : `Gleiches Filament wie Slot ${twin.slot}.`}{" "}
                      {byParts
                        ? `Teile von „${byParts.join("“, „")}“ nutzen Slot ${s.slot}. Wähle eine zweite Spule oder ändere die Teile in Bambu Studio.`
                        : s.spoolId ? "Eine Spule liegt nur in einem AMS-Fach. Lege die Slots zusammen oder wähle eine zweite Spule." : "Getrennt lassen, wenn zwei Spulen eingelegt werden."}{" "}
                      {!byParts && <button className="link-btn" onClick={() => mergeSlot(s.slot, twin.slot)}>Mit Slot {twin.slot} zusammenlegen</button>}
                    </p>
                  )}
                  {w.map((t) => <p key={t} className="warn small slot-note">{t}</p>)}
                  {sp && sp.inPrinter > 0 && !w.length && !twin && <p className="muted small slot-note">Liegt im Drucker {sp.printerName}{sp.amsSlot ? `, ${sp.amsSlot}` : ""}.</p>}
                </li>
              );
            })}
          </ul>
        )}
        {slots && usedSlots.length > 4 && <p className="muted small">{usedSlots.length} Filamente: Dafür brauchst du mehr als ein AMS mit 4 Plätzen.</p>}
        {slots && (
          <button className="ghost small-btn" onClick={addSlot} disabled={slots.length >= 16}><Plus size={12} /> Slot hinzufügen</button>
        )}

        {objects.length > 1 && slots && (
          <>
            <h5>Objekte</h5>
            <ul className="setup-objects">
              {objects.map((o) => {
                const fixed = layout?.objects.find((x) => x.index === o.index)?.fixed;
                const color = slots.find((s) => s.slot === o.slot)?.color;
                return (
                  <li key={o.index}>
                    <span className="swatches"><i style={{ background: color || "transparent" }} /></span>
                    <span className="grow">{o.name}</span>
                    {fixed && <span className="muted small" title="Teile dieses Objekts haben eigene Slots im Modell">Teile mit eigenen Farben</span>}
                    <select value={o.slot} onChange={(e) => setObjects((l) => l.map((x) => (x.index === o.index ? { ...x, slot: Number(e.target.value) } : x)))}
                      aria-label={`Slot für ${o.name}`}>
                      {slots.map((s) => <option key={s.slot} value={s.slot}>Slot {s.slot}{s.material ? ` · ${s.material}` : ""}</option>)}
                    </select>
                  </li>
                );
              })}
            </ul>
          </>
        )}
      </div>

      {msg && <p className="warn small">{msg}</p>}
      <div className="add-row">
        <button className={orderBasis === "tatsaechlich" || orderBasis === "slicer" ? "" : "primary"} onClick={slice} disabled={busy || incomplete}>
          <Calculator size={13} /> {busy ? "Slict …" : "Slicen und kalkulieren"}
        </button>
        <button onClick={() => save().then(onChanged).catch(fail)} disabled={busy || !process}><Save size={13} /> Speichern</button>
        <button className="ghost small-btn" onClick={() => loadCat(true)} title="Profile neu laden" aria-label="Profile neu laden"><RefreshCw size={12} /></button>
      </div>
      <p className="muted small">
        Profile aus {cat?.source ?? "der Profilquelle"}. Bei mehreren Slots rechnet die App mit dem Filament-Profil des ersten Slots für alle;
        die Mengen je Slot kommen aus dem Modell. Slicen startet Bambu Studio ohne Oberfläche und nur zum Berechnen:
        Es druckt nichts und legt nichts im Auftragsordner ab. Einschalten unter Einstellungen → Anbindungen → Bambu Studio.
      </p>
      <footer className="row-end">
        <button onClick={onClose}>Fertig</button>
      </footer>
    </Modal>
  );
}
