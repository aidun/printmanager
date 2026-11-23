// Nur für die Vorschau im Browser (npm run dev ohne Tauri): beantwortet die Befehle mit erfundenen Beispieldaten.
// Wird in main.tsx nur geladen, wenn die App nicht in Tauri läuft. Kommt nicht in den Build der App.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type {
  ConnectorWrite, ConnectorInfo, ContactHit, ItemEstimate, ProfileChoice, SetupObject, SetupSlot, AppInfo, Attachment, BambuStatus, HistoryEntry, PersonInput, CatalogEntry, Event, Item, MwCollection, OrderDetail, OrderSummary, Person,
  PrinterView, PrintRun, Spool, Stats, Status,
} from "./api";

const now = Date.now();
const iso = (days: number, hours = 0) => new Date(now + days * 86400000 + hours * 3600000).toISOString();
const day = (days: number) => iso(days).slice(0, 10);

const person = (id: string, number: number, name: string, email: string, phone: string, note = ""): Person => ({
  id, number, name, email, phone, street: "", houseNo: "", zip: "", city: "", billingDiffers: 0, billingName: "", billingStreet: "",
  billingHouseNo: "", billingZip: "", billingCity: "", note, createdAt: iso(-90 + number * 10), orderCount: 0, openCount: 0, lastOrderAt: null,
});
const people: Person[] = [
  { ...person("p1", 1, "Anna Berger", "anna@example.com", "0151 2345678", "mag Pastellfarben"), street: "Lindenweg", houseNo: "4", zip: "70173", city: "Stuttgart" },
  person("p2", 2, "Familie Krause", "", "0711 998877", "Nachbarn"),
  person("p3", 3, "Jonas Weber", "jonas@example.com", ""),
  { ...person("p4", 4, "Kita Sonnenschein", "kita@example.com", "", "Bastelprojekte"), street: "Schulstraße", houseNo: "12", zip: "70180", city: "Stuttgart",
    billingDiffers: 1, billingName: "Stadt Stuttgart – Jugendamt", billingStreet: "Wilhelmsplatz", billingHouseNo: "1", billingZip: "70182", billingCity: "Stuttgart" },
];

type O = [number, string, Status, number, number | null, string | null, number, number];
const raw: O[] = [
  [12, "Ersatzknopf Spülmaschine", "eingang", 1, 2, "p2", 1, 2],
  [13, "Namensschilder Gruppe Igel", "klaeren", 0, 6, "p4", 3, 4],
  [11, "Halterung für Fahrradlicht", "vorbereiten", 0, 9, "p3", 1, 3],
  [10, "Weihnachtsdeko Set", "in_studio", 2, -1, "p1", 4, 5],
  [9, "Kabelkanal Schreibtisch", "druckt", 0, 3, "p3", 2, 3],
  [8, "Pflanzentopf mit Untersetzer", "nacharbeit", 0, null, "p1", 1, 2],
  [7, "Schlüsselanhänger Logo", "fertig", 1, 1, "p2", 10, 2],
  [6, "Figuren Brettspiel", "uebergeben", 0, -10, "p1", 6, 4],
  [5, "Lampenschirm", "pausiert", 0, null, null, 1, 1],
];
const orders: OrderSummary[] = raw.map(([number, title, status, priority, due, pid, items, atts], i) => ({
  id: `o${number}`, number, title, status, priority, dueDate: due === null ? null : day(due), position: i,
  personId: pid, personName: people.find((p) => p.id === pid)?.name ?? null, itemCount: items, attachmentCount: atts,
  coverPath: null, printFileId: status === "in_studio" || status === "druckt" ? `a${number}` : null,
  runsOpen: status === "druckt" ? 1 : 0, runsUndeducted: status === "nacharbeit" ? 1 : 0, archivedAt: null,
  createdAt: iso(-number), updatedAt: iso(-i / 3),
}));

for (const p of people) {
  const mine = orders.filter((o) => o.personId === p.id);
  p.orderCount = mine.length;
  p.openCount = mine.filter((o) => !["uebergeben", "abgelehnt"].includes(o.status)).length;
  p.lastOrderAt = mine.map((o) => o.createdAt).sort().pop() ?? null;
}

const spool = (id: string, material: string, series: string, color: string, rest: number, extra: Partial<Spool> = {}): Spool => ({
  id, brand: "Bambu Lab", series, material, color, colorName: "", initialGrams: 1000, studioGrams: rest,
  remainingGrams: rest, studioStatus: "0", hidden: 0, missing: 0, note: "", updatedAt: iso(-1), source: "konto",
  sourceUpdatedAt: iso(-1), inPrinter: 0, printerName: "", amsSlot: "", depleted: 0, rfid: "", ...extra,
});
const spools: Spool[] = [
  spool("s1", "PLA", "PLA Matte", "#DE4343", 650, { inPrinter: 1, printerName: "H2C", amsSlot: "AMS 1 · Slot 2" }),
  spool("s2", "PLA", "PLA Basic", "#161616", 150, { inPrinter: 1, printerName: "H2C", amsSlot: "AMS 1 · Slot 1" }),
  spool("s3", "PETG", "PETG HF", "#F4F4F4", 1000, { inPrinter: 1, printerName: "H2C", amsSlot: "AMS 2 · Slot 3" }),
  spool("s4", "PETG", "PETG HF", "#2B5DA8", 820, { inPrinter: 1, printerName: "H2C", amsSlot: "AMS 2 · Slot 4" }),
  spool("s5", "PLA", "PLA Matte", "#0078BF", 340),
  spool("s6", "PLA", "PLA Silk", "#C9A227", 88),
  spool("s7", "PLA", "PLA Basic", "#FFFFFF", 910),
  spool("s8", "PLA", "PLA Matte", "#3F8E4F", 25),
  spool("s9", "TPU", "TPU 95A", "#E07B39", 480),
  spool("s10", "PLA", "PLA+ 2.0", "#FFF144", 58, { brand: "SUNLU" }),
  spool("s11", "ASA", "ASA", "#7A7A7A", 760),
];

const printers: PrinterView[] = [
  { id: "d1", name: "H2C", model: "H2C", online: true, state: "printing", rawStatus: "RUNNING", taskName: "A0009 kabelkanal_plate_1",
    progress: 46, remainingSeconds: 4870, progressEstimated: true, orderId: "o9", orderNumber: 9, orderTitle: "Kabelkanal Schreibtisch", matchedBy: "nummer", updatedAt: iso(0) },
  { id: "d2", name: "A1", model: "A1", online: true, state: "idle", rawStatus: "IDLE", taskName: null, progress: null,
    remainingSeconds: null, progressEstimated: false, orderId: null, orderNumber: null, orderTitle: null, matchedBy: null, updatedAt: iso(0) },
];

const svg = (hue: number, label: string) =>
  `data:image/svg+xml;utf8,${encodeURIComponent(
    `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 400 300'><rect width='400' height='300' fill='hsl(${hue} 35% 88%)'/><circle cx='200' cy='150' r='70' fill='hsl(${hue} 45% 62%)'/><text x='200' y='280' font-family='sans-serif' font-size='22' text-anchor='middle' fill='hsl(${hue} 30% 35%)'>${label}</text></svg>`,
  )}`;
const cat = (id: string, title: string, hue: number, extra: Partial<CatalogEntry> = {}): CatalogEntry => ({
  id, title, description: "", fileName: `${id}.3mf`, thumbName: null, sourceUrl: null, designer: "", license: "",
  plates: 1, printSeconds: 7200, weightGrams: 64, filaments: JSON.stringify([{ type: "PLA", color: "#DE4343", grams: 64 }]),
  tags: "", hidden: 0, createdAt: iso(-20), updatedAt: iso(-2), usageCount: 0, origin: "lokal", remoteId: null,
  coverUrl: svg(hue, title.split(" ")[0]), remoteMissing: 0, collectionIds: [], ...extra,
});
const catalog: CatalogEntry[] = [
  cat("c1", "3D Benchy", 10, { usageCount: 3 }),
  cat("c2", "Kabelclip 6 mm", 200, { printSeconds: 1500, weightGrams: 6 }),
  cat("c3", "Weihnachtsstern", 50, { origin: "makerworld", remoteId: "1", fileName: "", collectionIds: ["k1"], designer: "NowCraft3D" }),
  cat("c4", "Hogwarts Lesezeichen", 280, { origin: "makerworld", remoteId: "2", collectionIds: ["k2"], license: "Standard Digital File License" }),
  cat("c5", "Pflanzentopf Spirale", 130, { usageCount: 1, printSeconds: 16200, weightGrams: 210 }),
  cat("c6", "Wandhaken minimal", 240, { origin: "makerworld", remoteId: "3", fileName: "", collectionIds: ["k2"] }),
  cat("c7", "Schlüsselanhänger Puzzle", 330, { origin: "makerworld", remoteId: "4", fileName: "", collectionIds: ["k1", "k2"] }),
  cat("c8", "Zahnbürstenhalter", 170),
];
const collections: MwCollection[] = [
  { id: "k1", title: "Weihnachten", designCnt: 9, isDefault: 1, enabled: 1, syncedAt: iso(0) },
  { id: "k2", title: "Haushalt", designCnt: 42, isDefault: 0, enabled: 1, syncedAt: iso(0) },
];

function detail(id: string): OrderDetail {
  const o = orders.find((x) => x.id === id) ?? orders[0];
  const items: Item[] = Array.from({ length: Math.min(o.itemCount, 4) }, (_, i) => ({
    id: `${id}-i${i}`, orderId: id, position: i, title: ["Grundteil", "Deckel", "Halter", "Clip"][i], quantity: i === 0 ? 2 : 1,
    colorWish: ["PLA rot", "PETG weiß", "schwarz", ""][i], note: "", catalogId: i === 0 ? "c5" : null, done: i === 0 && o.status === "nacharbeit" ? 1 : 0,
    fileAttachmentId: null,
  }));
  const attachments: Attachment[] = o.attachmentCount
    ? [
        { id: `a${o.number}`, orderId: id, kind: "file", name: `${o.title.split(" ")[0].toLowerCase()}.3mf`, path: "x.3mf", url: null, body: null, createdAt: iso(-3) },
        { id: `l${o.number}`, orderId: id, kind: "link", name: "MakerWorld", path: null, url: "https://makerworld.com/en/models/1", body: null, createdAt: iso(-3) },
      ]
    : [];
  const runs: PrintRun[] = o.status === "druckt" || o.status === "nacharbeit"
    ? [{
        id: `r${o.number}`, orderId: id, itemId: null, attachmentId: `a${o.number}`, fileName: attachments[0]?.name ?? "datei.3mf",
        plates: 1, printSeconds: 9000, weightGrams: 84, filaments: JSON.stringify([{ type: "PLA", color: "#DE4343", grams: 61.2 }, { type: "PETG", color: "#F4F4F4", grams: 22.9 }]),
        result: "offen", deducted: 0, note: "", createdAt: iso(-0.2), updatedAt: iso(-0.1),
        printerName: o.status === "nacharbeit" ? "H2C" : "", printerStatus: o.status === "nacharbeit" ? "ok" : "",
        printerFinishedAt: null, actualSeconds: o.status === "nacharbeit" ? 9420 : null,
        actualFilaments: o.status === "nacharbeit" ? JSON.stringify([{ type: "PLA", color: "#DE4343", grams: 63.4 }]) : "[]",
        plateDetails: o.status === "druckt" ? JSON.stringify([1, 2, 3].map((index) => ({
          index, seconds: 3000, filaments: [{ type: "PLA", color: "#DE4343", grams: 20.4 }],
          status: plateState[`${o.number}-${index}`] ?? (index === 1 ? "ok" : ""),
          actualSeconds: index === 1 ? 3240 : null, actualFilaments: index === 1 ? [{ type: "PLA", color: "#DE4343", grams: 21.0 }] : [],
          taskId: index === 1 ? "t1" : null, printerName: index === 1 ? "H2C" : "", finishedAt: null,
        }))) : "[]",
      }]
    : [];
  const events: Event[] = [
    { id: "e1", orderId: id, kind: "status", fromStatus: "eingang", toStatus: o.status, body: "", createdAt: iso(-1) },
    { id: "e2", orderId: id, kind: "note", fromStatus: null, toStatus: null, body: "Farbe mit Kundin abgestimmt.", createdAt: iso(-2) },
    { id: "e3", orderId: id, kind: "created", fromStatus: null, toStatus: "eingang", body: "", createdAt: iso(-o.number) },
  ];
  return {
    order: { id, number: o.number, personId: o.personId, title: o.title, description: "Bitte möglichst stabil, wird draußen benutzt.",
      status: o.status, priority: o.priority, dueDate: o.dueDate, position: o.position, archivedAt: null, createdAt: o.createdAt, updatedAt: o.updatedAt },
    person: people.find((p) => p.id === o.personId) ?? null, items, attachments, events, runs, folder: `/Daten/orders/${o.number}`,
  };
}

const titles = ["Kabelclip_6mm_plate_1", "A0009 kabelkanal_plate_1", "Weihnachtsstern_plate_2", "benchy_plate_1", "Pflanzentopf Spirale",
  "Zahnbuerstenhalter_plate_1", "Hogwarts Bookmarks_plate_3", "Wandhaken_minimal", "Schluesselanhaenger_Logo"];
const history: HistoryEntry[] = Array.from({ length: 42 }, (_, i) => {
  const end = iso(-i * 3.3, -2);
  const secs = 1800 + ((i * 2777) % 18000);
  const t = titles[i % titles.length];
  const o = i === 1 ? orders.find((x) => x.number === 9) : i === 4 ? orders.find((x) => x.number === 8) : undefined;
  return {
    id: String(1449600000 + i), title: t, designTitle: i % 3 === 0 ? "MakerWorld: " + t.split("_")[0] : "", designId: null,
    deviceName: i % 4 === 3 ? "a1" : "h2c", deviceModel: "", plateIndex: 1, status: i % 11 === 5 ? 3 : 2,
    startTime: new Date(new Date(end).getTime() - secs * 1000).toISOString(), endTime: end, plannedSeconds: Math.round(secs * 0.93),
    weightGrams: 10 + (i * 13) % 140,
    filaments: JSON.stringify([{ type: i % 3 ? "PLA" : "PETG", color: ["#DE4343", "#161616", "#F4F4F4", "#2B5DA8"][i % 4], grams: 10 + (i * 13) % 140 }]),
    coverUrl: svg((i * 37) % 360, t.split(/[_ ]/)[0]), orderId: o?.id ?? null, matchedBy: o ? (i === 1 ? "nummer" : "manuell") : null,
    orderNumber: o?.number ?? null, orderTitle: o?.title ?? null, personName: o?.personName ?? null,
    progressAtEnd: null, usedFactor: i % 11 === 5 ? 0.2 : 1, usedMethod: i % 11 === 5 ? "zeit" : "fertig",
  };
});

const stats: Stats = {
  history,
  runs: Array.from({ length: 40 }, (_, i) => ({
    createdAt: iso(-i * 8), result: i % 9 === 4 ? "fehldruck" : "ok", printSeconds: 3000 + (i % 7) * 2400,
    filaments: JSON.stringify([{ type: i % 3 ? "PLA" : "PETG", color: "#DE4343", grams: 30 + (i % 5) * 25 }]),
    orderNumber: 1 + (i % 12), personName: people[i % 4].name,
  })),
  handovers: Array.from({ length: 14 }, (_, i) => ({ createdAt: iso(-i * 20), orderNumber: i + 1, personName: people[i % 4].name })),
};

const writes: ConnectorWrite[] = [
  { id: "w2", createdAt: iso(0, -2), connector: "bambu-cloud", entity: "spool", localId: "s2", label: "PLA Basic Weiß", action: "rest", oldValue: "612", newValue: "548", status: "ok", message: "" },
  { id: "w1", createdAt: iso(-1), connector: "bambu-cloud", entity: "spool", localId: "s4", label: "PETG HF Schwarz", action: "anlegen", oldValue: null, newValue: "1000", status: "ok", message: "" },
];

const cap = (cap: ConnectorInfo["caps"][number]["cap"], value = true, write = false) => ({ cap, value, write, on: value });
const ok = (account: string | null = null, detail: string | null = null) =>
  ({ connected: true, expired: false, account, lastOkAt: iso(0, -1), lastError: null, detail });
const connectors: ConnectorInfo[] = [
  {
    id: "bambu-cloud", name: "Bambu-Konto", kind: "printer-cloud", auth: "webview-cookie", enabled: true,
    caps: [cap("spools-read"), cap("spools-write", true, true), cap("printers-read"), cap("print-history-read"), cap("catalog-read")],
    status: ok("Testkonto", "gültig bis 15.02.2027"),
  },
  { id: "bambu-studio", name: "Bambu Studio", kind: "slicer", auth: "none", enabled: true, caps: [cap("slicer-open"), cap("spools-read"), cap("slicer-slice", true)], status: ok() },
  { id: "printerprofiles", name: "Druckprofile (aidun/printerprofiles)", kind: "slicer", auth: "none", enabled: true, caps: [cap("profiles-read")], status: ok() },
  { id: "web", name: "Web", kind: "catalog", auth: "none", enabled: true, caps: [cap("links-read")], status: ok() },
  { id: "printables", name: "Printables", kind: "catalog", auth: "none", enabled: true, caps: [cap("links-read")], status: ok() },
  {
    id: "apple-contacts", name: "Apple Kontakte (dieser Mac)", kind: "contacts", auth: "os-permission", enabled: true,
    caps: [cap("contacts-read")], status: { connected: true, expired: false, account: null, lastOkAt: null, lastError: null, detail: null },
  },
];

const estimates: ItemEstimate[] = [];
/** übersprungene Platten der Beispiel-Drucke („<Auftrag>-<Platte>“) */
const plateState: Record<string, string> = {};
const estFor = (itemId: string) => {
  let e = estimates.find((x) => x.itemId === itemId);
  if (!e) {
    e = { itemId, source: "", printer: "", nozzle: "", process: "", filament: "", bedType: "", basis: "", seconds: null, filaments: "[]", mwInstance: "", error: "", updatedAt: null,
      slots: "[]", objects: "[]", setupState: "", projectFile: "" };
    estimates.push(e);
  }
  return e;
};

/** erfundene Kontakte für die Vorschau */
const mockContacts: ContactHit[] = [
  { connector: "apple-contacts", source: "Apple Kontakte (dieser Mac)", externalId: "c1", name: "Clara Becker", organization: "", email: "clara@example.org", phone: "+49 170 5550101", city: "Ulm", personId: null },
  { connector: "apple-contacts", source: "Apple Kontakte (dieser Mac)", externalId: "c2", name: "Werkstatt Nord GmbH", organization: "Werkstatt Nord GmbH", email: "", phone: "0711 5550202", city: "Stuttgart", personId: null },
  { connector: "apple-contacts", source: "Apple Kontakte (dieser Mac)", externalId: "c3", name: "Anna Berger", organization: "", email: "anna@example.org", phone: "+49 151 5550303", city: "Esslingen", personId: "p1" },
];
const contactLinks: Record<string, { externalId: string; state: "ok" | "konflikt" }> = { p1: { externalId: "c3", state: "konflikt" } };

const bambu: BambuStatus = {
  region: "global", accountName: "Testkonto", uid: 1, expiresAt: iso(90), filament: true, printers: true, makerworld: true, filamentWrite: true,
  connected: true, expired: false, lastOkAt: iso(0), lastError: null, loggingIn: false, filamentSyncedAt: iso(0), mwSyncedAt: iso(0),
};

const info: AppInfo = {
  dataDir: "/Users/test/Library/Application Support/de.printmanager.app", defaultDataDir: "/Users/test/Library/Application Support/de.printmanager.app",
  database: "SQLite", settings: {}, statuses: [], studioFound: true, spoolsFile: "/Users/test/Library/Application Support/BambuStudio/filament_inventory/spools.json",
  defaultSpoolsFile: "", version: "0.1.0 (Vorschau)",
};

const pricing = {
  filamentEurPerKg: { default: 25, PLA: 20, PETG: 22, TPU: 32, ASA: 28 }, powerWatts: 150, powerEurPerKwh: 0.35,
  machineEurPerHour: 0.4, failurePct: 10, handlingEur: 2, markupFactor: 2.5, minPriceEur: 5, roundToEur: 0.5,
};

mockWindows("main");
mockIPC(
  (cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "app_info": return info;
      case "pricing_get": return pricing;
      case "pricing_save": Object.assign(pricing, a.pricing as object); return null;
      case "people_list": return people;
      case "history_list": return history;
      case "history_sync": return { fetched: history.length, added: 0, matched: 0, total: history.length };
      case "history_assign": {
        const h = history.find((x) => x.id === a.id);
        const o = orders.find((x) => x.id === a.orderId);
        if (h) Object.assign(h, o ? { orderId: o.id, orderNumber: o.number, orderTitle: o.title, personName: o.personName, matchedBy: "manuell" }
          : { orderId: null, orderNumber: null, orderTitle: null, personName: null, matchedBy: null });
        return null;
      }
      case "person_save": {
        const p = a.person as PersonInput;
        const name = p.name.trim();
        const twin = people.find((x) => x.name.toLowerCase() === name.toLowerCase() && x.id !== p.id);
        if (twin) throw `Den Kunden „${name}“ gibt es schon (K-${String(twin.number).padStart(4, "0")}).`;
        if (p.billingDiffers && (!p.billingStreet.trim() || !p.billingCity.trim())) throw "Für die Rechnungsadresse bitte mindestens Straße und Ort angeben.";
        const data = { ...p, name, billingDiffers: p.billingDiffers ? 1 : 0 };
        if (!p.billingDiffers) Object.assign(data, { billingName: "", billingStreet: "", billingHouseNo: "", billingZip: "", billingCity: "" });
        const old = people.find((x) => x.id === p.id);
        if (old) {
          Object.assign(old, data);
          return old.id;
        }
        const n = { ...person(`p${Date.now()}`, Math.max(0, ...people.map((x) => x.number)) + 1, name, p.email, p.phone, p.note), ...data, id: "" };
        n.id = `p${Date.now()}`;
        people.push(n);
        return n.id;
      }
      case "person_delete": {
        const i = people.findIndex((x) => x.id === a.id);
        if (i >= 0) people.splice(i, 1);
        return null;
      }
      case "order_create": {
        const inp = a.input as { title: string; personId?: string | null };
        if (!inp.personId) throw "Bitte einen Kunden angeben.";
        const number = Math.max(...orders.map((o) => o.number)) + 1;
        const p = people.find((x) => x.id === inp.personId);
        const o: OrderSummary = { ...orders[0], id: `o${number}`, number, title: inp.title, status: "eingang", priority: 0, dueDate: null,
          personId: inp.personId, personName: p?.name ?? null, itemCount: 0, attachmentCount: 0, printFileId: null, runsOpen: 0, runsUndeducted: 0,
          createdAt: iso(0), updatedAt: iso(0) };
        orders.unshift(o);
        if (p) { p.orderCount++; p.openCount++; p.lastOrderAt = o.createdAt; }
        return { ...detail(o.id).order };
      }
      case "orders_list": {
        const q = String(a.search ?? "").toLowerCase();
        return q ? orders.filter((o) => `${o.title} ${o.personName} ${o.number}`.toLowerCase().includes(q)) : orders;
      }
      case "order_get": return detail(String(a.id));
      case "order_set_status": {
        const o = orders.find((x) => x.id === a.id);
        if (o) o.status = a.status as Status;
        return null;
      }
      case "spools_list": return spools.map((x) => ({ ...x }));
      case "printers_list": return { enabled: true, connected: true, printers, updatedAt: iso(0), error: null };
      case "printer_assign": {
        const o = orders.find((x) => x.id === a.orderId);
        Object.assign(printers[0], o ? { orderId: o.id, orderNumber: o.number, orderTitle: o.title, matchedBy: "manuell" }
          : { orderId: "o9", orderNumber: 9, orderTitle: "Kabelkanal Schreibtisch", matchedBy: "nummer" });
        return { enabled: true, connected: true, printers, updatedAt: iso(0), error: null };
      }
      case "printers_refresh": return { enabled: true, connected: true, printers, updatedAt: iso(0), error: null };
      case "catalog_list": return catalog;
      case "mw_collections": return collections;
      case "stats": return stats;
      case "bambu_status": return bambu;
      case "connectors_list": return connectors.map((c) => ({ ...c, caps: c.caps.map((x) => ({ ...x, on: c.enabled && x.value })) }));
      case "connector_set_cap": {
        const c = connectors.find((x) => x.id === a.id)!;
        c.caps.find((x) => x.cap === a.cap)!.value = Boolean(a.on);
        if (c.id === "bambu-cloud") bambu.filamentWrite = c.caps.find((x) => x.cap === "spools-write")!.value;
        return null;
      }
      case "connector_set_enabled": connectors.find((x) => x.id === a.id)!.enabled = Boolean(a.enabled); return null;
      case "connector_connect": {
        const c = connectors.find((x) => x.id === a.id)!;
        Object.assign(c.status, { connected: true, expired: false, account: "Testkonto" });
        c.enabled = true;
        return null;
      }
      case "connector_disconnect": {
        const c = connectors.find((x) => x.id === a.id)!;
        Object.assign(c.status, { connected: false, account: null, detail: null, lastOkAt: null });
        c.enabled = false;
        return null;
      }
      case "connector_writes": return writes;
      case "slice_profiles": return {
        source: "printerprofiles",
        printers: [{ id: "h2c", label: "H2C", nozzles: ["0.2", "0.4", "0.6"] }, { id: "a1", label: "A1", nozzles: ["0.4"] }],
        presets: ["h2c", "a1"].flatMap((pr) => ["0.2", "0.4", "0.6"].flatMap((n) => [
          ...(["quality", "normal", "fast"] as const).flatMap((lv) => [false, true].map((sup) => ({
            kind: "process" as const, name: `${lv} ${pr} ${n}`, file: `process/${lv}${sup ? "+s" : ""} ${pr} ${n}.json`, printer: pr, nozzle: n,
            base: "", level: lv, support: sup, layerHeight: lv === "quality" ? "0.12" : lv === "normal" ? "0.2" : "0.28" }))),
          ...["Bambu PLA Basic", "Bambu PLA Matte", "Bambu PETG HF", "SUNLU PETG", "SUNLU PLA+ 2.0", "eSUN PLA+"].map((f) => ({
            kind: "filament" as const, name: `${f} ${pr.toUpperCase()} ${n}`, file: `filament/${f} ${pr} ${n}.json`, printer: pr, nozzle: n, base: "",
            filament: f.toLowerCase().replace(/\+/g, "-plus").replace(/[^a-z0-9]+/g, "-") })),
        ])),
      };
      case "item_estimates": return estimates;
      case "order_plates": {
        // Beispiel: das erste Teil hat eine Projektdatei mit drei Platten
        const itemId = `${a.orderId}-i0`;
        const img = (fill: string, label: string) => `data:image/svg+xml;utf8,${encodeURIComponent(
          `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' fill='#e9e9ee'/><rect x='14' y='14' width='72' height='72' rx='4' fill='#d4d4dc'/><circle cx='42' cy='46' r='16' fill='${fill}'/><rect x='56' y='52' width='20' height='20' rx='3' fill='${fill}' opacity='.7'/><text x='50' y='94' font-size='9' text-anchor='middle' fill='#666'>${label}</text></svg>`)}`;
        const e = estimates.find((x) => x.itemId === itemId);
        return [{
          itemId, fileName: "grundteil.3mf",
          plates: [{ index: 1, name: "Deckel", image: img("#DE4343", "1") }, { index: 2, name: "", image: img("#F4F4F4", "2") }, { index: 3, name: "Ersatzteile", image: null }],
          chosen: e?.plates ? JSON.parse(e.plates) : [],
        }];
      }
      case "order_plates_sync": return false;
      case "item_set_plates": {
        const e = estFor(String(a.itemId));
        const plates = a.plates as number[];
        e.plates = JSON.stringify(plates);
        if (e.basis === "slicer") e.seconds = Math.round(9420 * (plates.length ? plates.length / 3 : 1));
        return { ...e };
      }
      case "run_plate_skip": {
        const n = String(a.id).replace(/^r/, "");
        if (a.skip) plateState[`${n}-${a.index}`] = "uebersprungen"; else delete plateState[`${n}-${a.index}`];
        return null;
      }
      case "item_set_profile": {
        const e = estFor(String(a.itemId));
        const { slots, objects, plates, ...p } = a.profile as ProfileChoice;
        Object.assign(e, p);
        if (plates) e.plates = JSON.stringify(plates);
        if (slots) e.slots = JSON.stringify(slots);
        if (objects) e.objects = JSON.stringify(objects);
        const sl: SetupSlot[] = JSON.parse(e.slots);
        const ob: SetupObject[] = JSON.parse(e.objects);
        const used = sl.filter((s) => !ob.length || ob.some((o) => o.slot === s.slot));
        e.setupState = !sl.length ? "" : e.process && used.every((s) => s.preset) ? "bereit" : "entwurf";
        return { ...e };
      }
      case "item_setup": {
        const e = estFor(String(a.itemId));
        const layout = String(a.itemId).endsWith("-i0") ? {
          slots: [{ slot: 1, material: "PLA", color: "#DE4343", name: "Bambu PLA Basic @BBL H2C" }, { slot: 2, material: "PLA", color: "#F2F2F2", name: "Bambu PLA Basic @BBL H2C" }],
          objects: [{ index: 1, name: "Deckel", slot: 1, fixed: false, parts: [1] }, { index: 2, name: "Boden", slot: 2, fixed: false, parts: [2] }, { index: 3, name: "Griff", slot: 1, fixed: true, parts: [1, 2] }],
          painted: false,
          plates: [{ index: 1, name: "Deckel" }, { index: 2, name: "" }, { index: 3, name: "Ersatzteile" }],
        } : null;
        return { estimate: { ...e }, layout, layoutError: layout ? null : "Das Teil hat noch keine Modelldatei (STL oder 3MF) zum Slicen." };
      }
      case "item_slice": {
        const e = estFor(String(a.itemId));
        const sl: SetupSlot[] = JSON.parse(e.slots);
        const fil = sl.length
          ? sl.map((s, i) => ({ type: s.material, color: s.color, grams: i === 0 ? 61.4 : 22.8, spoolId: s.spoolId, slot: s.slot }))
          : [{ type: "PLA", color: "#DE4343", grams: 84.2 }];
        Object.assign(e, { basis: "slicer", seconds: 9420, filaments: JSON.stringify(fil), error: "" });
        return { ...e };
      }
      case "item_makerworld_profiles": return [
        { id: "m1", title: "0.2 mm Standard", seconds: 7200, grams: 64.5, default: true },
        { id: "m2", title: "0.16 mm fein", seconds: 10800, grams: 66, default: false },
      ];
      case "item_estimate_makerworld": {
        const e = estFor(String(a.itemId));
        Object.assign(e, { basis: "makerworld", seconds: 7200, mwInstance: a.profileId, filaments: JSON.stringify([{ type: "PLA", color: "", grams: 64.5 }]), error: "" });
        return e;
      }
      case "contacts_search": {
        const q = String(a.query ?? "").toLowerCase();
        return mockContacts.filter((c) => !q || `${c.name} ${c.email} ${c.phone}`.toLowerCase().includes(q));
      }
      case "person_from_contact": {
        const c = mockContacts.find((x) => x.externalId === a.externalId)!;
        if (c.personId) return { personId: c.personId, message: "Dieser Kontakt ist schon als Kunde übernommen." };
        const id = `p${Date.now()}`;
        people.push({ ...person(id, Math.max(0, ...people.map((x) => x.number)) + 1, c.name, c.email, c.phone), city: c.city });
        c.personId = id;
        contactLinks[id] = { externalId: c.externalId, state: "ok" };
        return { personId: id, message: `Kunde „${c.name}“ aus dem Kontakt angelegt.` };
      }
      case "person_link_contact": {
        contactLinks[String(a.personId)] = { externalId: String(a.externalId), state: "ok" };
        mockContacts.find((x) => x.externalId === a.externalId)!.personId = String(a.personId);
        return "Mit dem Kontakt verknüpft, ergänzt: Ort.";
      }
      case "person_unlink_contact": delete contactLinks[String(a.personId)]; return null;
      case "person_contact": {
        const l = contactLinks[String(a.personId)];
        if (!l) return null;
        return {
          connector: "apple-contacts", source: "Apple Kontakte (dieser Mac)", externalId: l.externalId, state: l.state, lastSyncedAt: iso(0, -1),
          conflicts: l.state === "konflikt" ? [{ field: "phone", label: "Telefon", contact: "+49 151 5550303", customer: "0151 5550399" }] : [],
        };
      }
      case "person_contact_resolve": contactLinks[String(a.personId)].state = "ok"; return null;
      case "contacts_sync_now": return ["Anna Berger: Kontakt und Kunde wurden beide geändert – bitte beim Kunden entscheiden"];
      case "contacts_probe": return {
        count: 214, withName: 201, onlyOrganization: 13, withEmail: 120, withPhone: 180, withAddress: 64,
        streetWithNumber: 58, streetWithoutNumber: 6, streetMultiline: 2,
        phoneFormats: { international: 150, national: 40, sonstige: 2 }, kinds: { mobil: 140, privat: 90, arbeit: 50, sonstige: 12 },
        countries: { DE: 60, AT: 3, ohne: 1 },
      };
      case "spool_push_rest": {
        const sp = spools.find((x) => x.id === a.id);
        if (sp?.rfid) return { status: "rfid", message: "RFID-Spule: den Rest pflegt das AMS", cloudGrams: null };
        if (sp && !a.force && sp.id === spools[1].id) return { status: "konflikt", message: `Im Konto steht inzwischen ${sp.studioGrams - 40} g`, cloudGrams: sp.studioGrams - 40 };
        if (sp) { writes.unshift({ id: `w${writes.length + 1}`, createdAt: iso(0), connector: "bambu-cloud", entity: "spool", localId: sp.id, label: `${sp.material} ${sp.series}`, action: "rest", oldValue: String(sp.studioGrams), newValue: String(a.grams), status: "ok", message: "" }); sp.studioGrams = Number(a.grams); sp.remainingGrams = Number(a.grams); }
        return { status: "ok", message: "Im Bambu-Konto gespeichert", cloudGrams: a.grams };
      }
      case "spool_assign_slot": {
        const sp = spools.find((x) => x.id === a.id)!;
        const p = printers.find((x) => x.id === a.printer);
        const ams = Number(a.ams), slot = Number(a.slot);
        Object.assign(sp, p ? { inPrinter: 1, printerName: p.name, amsSlot: ams === 254 ? "Externe Spule" : `AMS ${ams + 1} · Slot ${slot + 1}` } : { inPrinter: 0, printerName: "", amsSlot: "" });
        return { status: "ok", message: p ? `Im Konto ${p.name} · ${sp.amsSlot}` : "Aus dem Drucker genommen", cloudGrams: null };
      }
      case "spool_create": {
        const n = a.spool as { brand: string; material: string; series: string; color: string; name: string; totalGrams: number; remainingGrams: number; count: number };
        for (let i = 0; i < n.count; i++) {
          spools.push(spool(`new${spools.length}`, n.material, n.series || n.material, n.color, n.remainingGrams, { brand: n.brand, colorName: n.name, initialGrams: n.totalGrams, studioGrams: n.remainingGrams }));
        }
        return { status: "ok", message: `${n.count} Spule(n) im Bambu-Konto angelegt`, cloudGrams: null };
      }
      case "spool_stock_add": {
        const sp = spools.find((x) => x.id === a.id)!;
        spools.push({ ...sp, id: `${sp.id}-${spools.length}`, remainingGrams: sp.initialGrams, studioGrams: sp.initialGrams, inPrinter: 0, printerName: "", amsSlot: "", depleted: 0 });
        return { status: "ok", message: "Spule im Bambu-Konto angelegt", cloudGrams: sp.initialGrams };
      }
      case "spool_stock_remove": {
        const i = spools.findIndex((x) => x.id === a.id);
        if (i >= 0) spools.splice(i, 1);
        return { status: "ok", message: "Spule im Bambu-Konto gelöscht", cloudGrams: null };
      }
      case "connector_write_undo": return { status: "ok", message: "Alter Wert zurückgeschrieben", cloudGrams: null };
      case "filament_sync": return { source: "konto", path: "Bambu-Konto", found: true, added: 0, updated: 0, missing: 0 };
      case "mw_sync": return { collections: 2, designs: 51, added: 0, missing: 0 };
      case "plugin:event|listen": return 1;
      case "plugin:event|unlisten": return null;
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);
