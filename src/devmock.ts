// Nur für die Vorschau im Browser (npm run dev ohne Tauri): beantwortet die Befehle mit erfundenen Beispieldaten.
// Wird in main.tsx nur geladen, wenn die App nicht in Tauri läuft. Kommt nicht in den Build der App.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type {
  AppInfo, Attachment, BambuStatus, CatalogEntry, Event, Item, MwCollection, OrderDetail, OrderSummary, Person,
  PrinterView, PrintRun, Spool, Stats, Status,
} from "./api";

const now = Date.now();
const iso = (days: number, hours = 0) => new Date(now + days * 86400000 + hours * 3600000).toISOString();
const day = (days: number) => iso(days).slice(0, 10);

const people: Person[] = [
  { id: "p1", name: "Anna Berger", contact: "anna@example.com", note: "", createdAt: iso(-90), orderCount: 3 },
  { id: "p2", name: "Familie Krause", contact: "", note: "Nachbarn", createdAt: iso(-60), orderCount: 2 },
  { id: "p3", name: "Jonas Weber", contact: "0170 1234567", note: "", createdAt: iso(-40), orderCount: 2 },
  { id: "p4", name: "Kita Sonnenschein", contact: "", note: "Bastelprojekte", createdAt: iso(-30), orderCount: 1 },
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

const spool = (id: string, material: string, series: string, color: string, rest: number, extra: Partial<Spool> = {}): Spool => ({
  id, brand: "Bambu Lab", series, material, color, colorName: "", initialGrams: 1000, studioGrams: rest,
  remainingGrams: rest, studioStatus: "0", hidden: 0, missing: 0, note: "", updatedAt: iso(-1), source: "konto",
  sourceUpdatedAt: iso(-1), inPrinter: 0, printerName: "", amsSlot: "", depleted: 0, ...extra,
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
  { id: "d1", name: "H2C", model: "H2C", online: true, state: "printing", rawStatus: "RUNNING", taskName: "kabelkanal_plate_1",
    progress: 46, remainingSeconds: 4870, orderId: "o9", orderNumber: 9, orderTitle: "Kabelkanal Schreibtisch", updatedAt: iso(0) },
  { id: "d2", name: "A1", model: "A1", online: true, state: "idle", rawStatus: "IDLE", taskName: null, progress: null,
    remainingSeconds: null, orderId: null, orderNumber: null, orderTitle: null, updatedAt: iso(0) },
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
    colorWish: ["PLA rot", "PETG weiß", "schwarz", ""][i], note: "", catalogId: null, done: i === 0 && o.status === "nacharbeit" ? 1 : 0,
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

const stats: Stats = {
  runs: Array.from({ length: 40 }, (_, i) => ({
    createdAt: iso(-i * 8), result: i % 9 === 4 ? "fehldruck" : "ok", printSeconds: 3000 + (i % 7) * 2400,
    filaments: JSON.stringify([{ type: i % 3 ? "PLA" : "PETG", color: "#DE4343", grams: 30 + (i % 5) * 25 }]),
    orderNumber: 1 + (i % 12), personName: people[i % 4].name,
  })),
  handovers: Array.from({ length: 14 }, (_, i) => ({ createdAt: iso(-i * 20), orderNumber: i + 1, personName: people[i % 4].name })),
};

const bambu: BambuStatus = {
  region: "global", accountName: "Testkonto", uid: 1, expiresAt: iso(90), filament: true, printers: true, makerworld: true,
  connected: true, expired: false, lastOkAt: iso(0), lastError: null, loggingIn: false, filamentSyncedAt: iso(0), mwSyncedAt: iso(0),
};

const info: AppInfo = {
  dataDir: "/Users/test/Library/Application Support/de.printmanager.app", defaultDataDir: "/Users/test/Library/Application Support/de.printmanager.app",
  database: "SQLite", settings: {}, statuses: [], studioFound: true, spoolsFile: "/Users/test/Library/Application Support/BambuStudio/filament_inventory/spools.json",
  defaultSpoolsFile: "", version: "0.1.0 (Vorschau)",
};

mockWindows("main");
mockIPC(
  (cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "app_info": return info;
      case "people_list": return people;
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
      case "spools_list": return spools;
      case "printers_list": return { enabled: true, connected: true, printers, updatedAt: iso(0), error: null };
      case "printers_refresh": return { enabled: true, connected: true, printers, updatedAt: iso(0), error: null };
      case "catalog_list": return catalog;
      case "mw_collections": return collections;
      case "stats": return stats;
      case "bambu_status": return bambu;
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
