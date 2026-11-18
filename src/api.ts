import { invoke as tauriInvoke, convertFileSrc, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";

/** Befehle, die nur lesen. Alle anderen ändern Daten und lösen danach `pm:changed` aus. */
const READ_ONLY = new Set([
  "app_info", "people_list", "orders_list", "order_get", "catalog_list", "link_preview",
  "attachment_open", "order_reveal", "open_url", "spools_list", "mw_collections", "history_list", "connector_writes", "connectors_list", "stats", "bambu_status", "printers_list",
]);

/** Ereignis, auf das alle Ansichten mit Neuladen reagieren */
export const CHANGED = "pm:changed";

async function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  const result = await tauriInvoke<T>(cmd, args, options);
  if (!READ_ONLY.has(cmd)) window.dispatchEvent(new Event(CHANGED));
  return result;
}

export type Status =
  | "eingang" | "klaeren" | "vorbereiten" | "in_studio" | "druckt"
  | "nacharbeit" | "fertig" | "uebergeben" | "abgelehnt" | "pausiert";

/** Kunde (technisch weiter „Person“) */
export interface Person {
  id: string; number: number; name: string; email: string; phone: string;
  street: string; houseNo: string; zip: string; city: string;
  /** 1 = abweichende Rechnungsadresse */
  billingDiffers: number; billingName: string; billingStreet: string; billingHouseNo: string; billingZip: string; billingCity: string;
  note: string; createdAt: string; orderCount: number; openCount: number; lastOrderAt: string | null;
}
export interface PersonInput {
  id?: string; name: string; email: string; phone: string;
  street: string; houseNo: string; zip: string; city: string;
  billingDiffers: boolean; billingName: string; billingStreet: string; billingHouseNo: string; billingZip: string; billingCity: string;
  note: string;
}
export const EMPTY_PERSON: PersonInput = {
  name: "", email: "", phone: "", street: "", houseNo: "", zip: "", city: "",
  billingDiffers: false, billingName: "", billingStreet: "", billingHouseNo: "", billingZip: "", billingCity: "", note: "",
};
export function personInput(p: Person): PersonInput {
  const { id, name, email, phone, street, houseNo, zip, city, billingName, billingStreet, billingHouseNo, billingZip, billingCity, note } = p;
  return { id, name, email, phone, street, houseNo, zip, city, billingDiffers: !!p.billingDiffers, billingName, billingStreet, billingHouseNo, billingZip, billingCity, note };
}
/** Adresse in einer Zeile: „Hauptstr. 5, 12345 Berlin“ */
export function formatAddress(street: string, houseNo: string, zip: string, city: string) {
  const l1 = [street, houseNo].map((x) => x.trim()).filter(Boolean).join(" ");
  const l2 = [zip, city].map((x) => x.trim()).filter(Boolean).join(" ");
  return [l1, l2].filter(Boolean).join(", ");
}

/** Kundennummer zum Anzeigen, z.B. K-0007 */
export function customerNo(n: number) {
  return `K-${String(n).padStart(4, "0")}`;
}

export interface OrderSummary {
  id: string; number: number; title: string; status: Status; priority: number;
  dueDate: string | null; position: number; personId: string | null; personName: string | null;
  itemCount: number; attachmentCount: number; coverPath: string | null; printFileId: string | null;
  runsOpen: number; runsUndeducted: number; archivedAt: string | null;
  createdAt: string; updatedAt: string;
}

export interface StatRun {
  createdAt: string; result: RunResult; printSeconds: number | null; filaments: string;
  orderNumber: number; personName: string | null;
}
export interface Stats {
  runs: StatRun[];
  handovers: { createdAt: string; orderNumber: number; personName: string | null }[];
  /** alle Drucke aus dem Druckverlauf des Kontos */
  history: HistoryEntry[];
}

/** Ein Druck aus dem Druckverlauf des Bambu-Kontos (Gramm = Slicer-Werte, Dauer echt) */
export interface HistoryEntry {
  id: string; title: string; designTitle: string; designId: string | null; deviceName: string; deviceModel: string;
  plateIndex: number | null; status: number; startTime: string | null; endTime: string | null;
  plannedSeconds: number | null; weightGrams: number | null; filaments: string; coverUrl: string | null;
  orderId: string | null; matchedBy: string | null; orderNumber: number | null; orderTitle: string | null; personName: string | null;
  progressAtEnd: number | null; usedFactor: number | null; usedMethod: string | null;
}
export interface HistorySyncResult { fetched: number; added: number; matched: number; total: number }

/** Gramm eines Drucks aus dem Verlauf: bei Abbrüchen der geschätzte tatsächliche Verbrauch */
export function historyGrams(h: HistoryEntry) {
  const planned = parseRunFilaments(h.filaments).reduce((a, f) => a + f.grams, 0);
  return { planned, used: planned * (h.status === 3 ? h.usedFactor ?? 1 : 1) };
}
export const USAGE_METHOD: Record<string, string> = {
  fortschritt: "geschätzt aus dem Fortschritt beim Abbruch",
  zeit: "geschätzt aus der Laufzeit im Verhältnis zur geplanten Druckzeit",
  geplant: "geplanter Wert (keine Angaben für eine Schätzung)",
  fertig: "vollständig gedruckt",
};

/** echte Dauer eines Drucks aus dem Verlauf in Sekunden */
export function historySeconds(h: HistoryEntry) {
  if (!h.startTime || !h.endTime) return null;
  const d = (new Date(h.endTime).getTime() - new Date(h.startTime).getTime()) / 1000;
  return d > 0 ? d : null;
}

export interface Order {
  id: string; number: number; personId: string | null; title: string; description: string;
  status: Status; priority: number; dueDate: string | null; position: number; archivedAt: string | null;
  createdAt: string; updatedAt: string;
}

export interface Item {
  id: string; orderId: string; position: number; title: string; quantity: number;
  colorWish: string; note: string; catalogId: string | null; done: number; fileAttachmentId: string | null;
}

export interface Attachment {
  id: string; orderId: string; kind: "image" | "file" | "link" | "text"; name: string;
  path: string | null; url: string | null; body: string | null; createdAt: string;
}

export interface Event {
  id: string; orderId: string; kind: "created" | "status" | "note" | "attachment" | "edit" | "print";
  fromStatus: Status | null; toStatus: Status | null; body: string; createdAt: string;
}

export interface OrderDetail {
  order: Order; person: Person | null; items: Item[]; attachments: Attachment[]; events: Event[];
  runs: PrintRun[]; folder: string;
}

export interface Spool {
  id: string; brand: string; series: string; material: string; color: string; colorName: string;
  initialGrams: number; studioGrams: number; remainingGrams: number; studioStatus: string;
  hidden: number; missing: number; note: string; updatedAt: string;
  /** datei | konto */
  source: string; sourceUpdatedAt: string | null; inPrinter: number; printerName: string; amsSlot: string; depleted: number;
  rfid: string;
}

/** Ergebnis eines Schreibvorgangs ins Bambu-Konto */
export interface WriteResult { status: "ok" | "konflikt" | "rfid" | "wartet" | "fehler"; message: string; cloudGrams: number | null }
export interface NewSpool {
  brand: string; material: string; series: string; color: string; name: string;
  totalGrams: number; remainingGrams: number; note: string; count: number;
}

/** Ein Schreibvorgang bei einer Anbindung (Protokoll) */
export interface ConnectorWrite {
  id: string; createdAt: string; connector: string; entity: string; localId: string; label: string; action: string;
  oldValue: string | null; newValue: string | null; status: string; message: string;
}

/** Fähigkeit einer Anbindung (siehe connectors/mod.rs) */
export type Cap =
  | "spools-read" | "spools-write" | "printers-read" | "print-history-read"
  | "catalog-read" | "links-read" | "contacts-read" | "slicer-open";
export type ConnectorKind = "printer-cloud" | "slicer" | "catalog" | "contacts" | "storage";
export type AuthKind = "none" | "webview-cookie" | "o-auth-pkce" | "app-password" | "os-permission";
export interface ConnectorStatus {
  connected: boolean; expired: boolean; account: string | null;
  lastOkAt: string | null; lastError: string | null; detail: string | null;
}
export interface ConnectorInfo {
  id: string; name: string; kind: ConnectorKind; auth: AuthKind; enabled: boolean;
  caps: { cap: Cap; write: boolean; on: boolean; value: boolean }[];
  status: ConnectorStatus;
}

export interface SyncResult { source: string; path: string; found: boolean; added: number; updated: number; missing: number }

export type RunResult = "offen" | "ok" | "fehldruck";
export interface RunFilament { type: string; color: string; grams: number; spoolId?: string | null }
export interface PrintRun {
  id: string; orderId: string; itemId: string | null; attachmentId: string | null; fileName: string;
  plates: number; printSeconds: number | null; weightGrams: number | null; filaments: string;
  result: RunResult; deducted: number; note: string; createdAt: string; updatedAt: string;
  /** Rückmeldung des Druckers über das Bambu-Konto (nur Vorschlag) */
  printerName: string; printerStatus: "" | "ok" | "fehldruck"; printerFinishedAt: string | null;
  actualSeconds: number | null; actualFilaments: string;
}

export type PrinterState = "offline" | "idle" | "printing" | "paused" | "finished" | "failed";
export interface PrinterView {
  id: string; name: string; model: string; online: boolean; state: PrinterState; rawStatus: string;
  taskName: string | null; progress: number | null; remainingSeconds: number | null;
  orderId: string | null; orderNumber: number | null; orderTitle: string | null;
  /** wie der Auftrag gefunden wurde */
  matchedBy: "manuell" | "nummer" | "datei" | "titel" | null;
  updatedAt: string;
}
export interface PrintersResult {
  enabled: boolean; connected: boolean; printers: PrinterView[]; updatedAt: string | null; error: string | null;
}
export const PRINTER_STATE: Record<PrinterState, string> = {
  offline: "Offline", idle: "Bereit", printing: "Druckt", paused: "Pausiert", finished: "Fertig", failed: "Fehler",
};

export interface Filament { type: string; color: string; grams: number | null }

export interface CatalogEntry {
  id: string; title: string; description: string; fileName: string; thumbName: string | null;
  sourceUrl: string | null; designer: string; license: string; plates: number;
  printSeconds: number | null; weightGrams: number | null; filaments: string; tags: string; hidden: number;
  createdAt: string; updatedAt: string; usageCount: number;
  /** lokal | makerworld */
  origin: string; remoteId: string | null; coverUrl: string | null; remoteMissing: number; collectionIds: string[];
}

export interface MwCollection { id: string; title: string; designCnt: number; isDefault: number; enabled: number; syncedAt: string | null }
export interface MwSyncResult { collections: number; designs: number; added: number; missing: number }

export interface RemoteFile { id: string; name: string; size: number | null; kind: string }
export interface RemoteProfile { title: string; printSeconds: number | null; weightGrams: number | null }
export interface LinkPreview {
  source: "printables" | "makerworld" | "thingiverse" | "web"; sourceLabel: string; url: string; remoteId: string | null;
  title: string; description: string; imageUrl: string | null; designer: string; license: string;
  files: RemoteFile[]; profiles: RemoteProfile[]; note: string | null;
}

export interface Settings {
  dataDir?: string | null; databaseUrl?: string | null; bambuStudioPath?: string | null; spoolsPath?: string | null;
}

export interface AppInfo {
  dataDir: string; defaultDataDir: string; database: string; settings: Settings;
  statuses: Status[]; studioFound: boolean; spoolsFile: string; defaultSpoolsFile: string; version: string;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  saveSettings: (settings: Settings) => invoke<void>("settings_save", { settings }),

  people: () => invoke<Person[]>("people_list"),
  savePerson: (person: PersonInput) => invoke<string>("person_save", { person }),
  deletePerson: (id: string) => invoke<void>("person_delete", { id }),

  archive: (id: string, archived: boolean) => invoke<void>("order_archive", { id, archived }),
  archiveClosed: (days: number) => invoke<number>("orders_archive_closed", { days }),
  stats: () => invoke<Stats>("stats"),
  bambuStatus: () => invoke<BambuStatus>("bambu_status"),
  history: () => invoke<HistoryEntry[]>("history_list"),
  spoolPushRest: (id: string, grams: number, force = false) => invoke<WriteResult>("spool_push_rest", { id, grams, force }),
  spoolStockAdd: (id: string) => invoke<WriteResult>("spool_stock_add", { id }),
  spoolStockRemove: (id: string) => invoke<WriteResult>("spool_stock_remove", { id }),
  /** Neue Spule(n) im Bambu-Konto anlegen */
  spoolCreate: (spool: NewSpool) => invoke<WriteResult>("spool_create", { spool }),
  /** Spule im Konto einem Drucker/AMS-Slot zuordnen; printer = null löst die Zuordnung */
  spoolAssignSlot: (id: string, printer: string | null, ams: number, slot: number) =>
    invoke<WriteResult>("spool_assign_slot", { id, printer, ams, slot }),
  connectorWrites: () => invoke<ConnectorWrite[]>("connector_writes"),
  connectorWriteUndo: (writeId: string) => invoke<WriteResult>("connector_write_undo", { writeId }),
  connectors: () => invoke<ConnectorInfo[]>("connectors_list"),
  /** Anmelden; bei Bambu öffnet sich ein Fenster, der Rest läuft im Hintergrund (Ereignis pm:external) */
  connectorConnect: (id: string) => tauriInvoke<void>("connector_connect", { id }),
  connectorDisconnect: (id: string) => invoke<void>("connector_disconnect", { id }),
  connectorSetEnabled: (id: string, enabled: boolean) => invoke<void>("connector_set_enabled", { id, enabled }),
  connectorSetCap: (id: string, cap: Cap, on: boolean) => invoke<void>("connector_set_cap", { id, cap, on }),
  historySync: () => invoke<HistorySyncResult>("history_sync"),
  historyAssign: (id: string, orderId: string | null) => invoke<void>("history_assign", { id, orderId }),
  printers: () => invoke<PrintersResult>("printers_list"),
  printersRefresh: () => tauriInvoke<PrintersResult>("printers_refresh"),
  /** Druckauftrag von Hand einem Auftrag zuordnen; null = wieder automatisch */
  printerAssign: (taskName: string, orderId: string | null) => invoke<PrintersResult>("printer_assign", { taskName, orderId }),
  orders: (search?: string) => invoke<OrderSummary[]>("orders_list", { search: search || null }),
  order: (id: string) => invoke<OrderDetail>("order_get", { id }),
  createOrder: (input: {
    title: string; description?: string; personId?: string | null; personName?: string | null;
    priority?: number; dueDate?: string | null;
  }) => invoke<Order>("order_create", { input }),
  updateOrder: (update: {
    id: string; title: string; description: string; personId: string | null; priority: number; dueDate: string | null;
  }) => invoke<void>("order_update", { update }),
  setStatus: (id: string, status: Status, note?: string) =>
    invoke<void>("order_set_status", { id, status, note: note || null }),
  deleteOrder: (id: string) => invoke<void>("order_delete", { id }),
  addNote: (orderId: string, body: string) => invoke<void>("note_add", { orderId, body }),

  saveItem: (item: Partial<Item> & { orderId: string; title: string; quantity: number }) =>
    invoke<string>("item_save", {
      item: { id: "", position: 0, colorWish: "", note: "", catalogId: null, done: 0, fileAttachmentId: null, ...item },
    }),
  deleteItem: (id: string) => invoke<void>("item_delete", { id }),

  addPaths: (orderId: string, paths: string[]) => invoke<number>("attachment_add_paths", { orderId, paths }),
  addBytes: (orderId: string, name: string, bytes: Uint8Array) =>
    invoke<string>("attachment_add_bytes", bytes, {
      headers: { "x-order-id": encodeURIComponent(orderId), "x-name": encodeURIComponent(name) },
    }),
  addLink: (orderId: string, url: string, name?: string) =>
    invoke<string>("attachment_add_link", { orderId, url, name: name || null }),
  addText: (orderId: string, body: string) => invoke<string>("attachment_add_text", { orderId, body }),
  deleteAttachment: (id: string) => invoke<void>("attachment_delete", { id }),
  openAttachment: (id: string) => invoke<void>("attachment_open", { id }),
  /** Liefert den neuen Status, falls der Auftrag dabei auf „In Studio“ vorgerückt ist */
  openInStudio: (id: string) => invoke<string | null>("attachment_open_in_studio", { id }),
  revealOrder: (id: string) => invoke<void>("order_reveal", { id }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  mwCollections: () => invoke<MwCollection[]>("mw_collections"),
  mwCollectionToggle: (id: string, enabled: boolean) => invoke<void>("mw_collection_toggle", { id, enabled }),
  mwSync: () => invoke<MwSyncResult>("mw_sync"),
  /** 3MF eines MakerWorld-Eintrags laden (bei Bedarf) */
  catalogFetchFile: (id: string) => invoke<void>("catalog_fetch_file", { id }),
  catalog: (includeHidden = false) => invoke<CatalogEntry[]>("catalog_list", { includeHidden }),
  catalogImportPaths: (paths: string[]) => invoke<string[]>("catalog_import_paths", { paths }),
  catalogImportLink: (preview: LinkPreview, fileIds: string[]) => invoke<string[]>("catalog_import_link", { preview, fileIds }),
  catalogUpdate: (update: { id: string; title: string; description: string; tags: string; hidden: number }) =>
    invoke<void>("catalog_update", { update }),
  catalogDelete: (id: string) => invoke<void>("catalog_delete", { id }),
  catalogOpenInStudio: (id: string) => invoke<void>("catalog_open_in_studio", { id }),
  catalogAddToOrder: (orderId: string, catalogId: string, quantity: number, colorWish: string) =>
    invoke<string>("catalog_add_to_order", { orderId, catalogId, quantity, colorWish }),

  linkPreview: (url: string) => invoke<LinkPreview>("link_preview", { url }),
  linkImport: (orderId: string, preview: LinkPreview, fileIds: string[]) =>
    invoke<number>("link_import_to_order", { orderId, preview, fileIds }),

  spools: () => invoke<Spool[]>("spools_list"),
  /** Konto, falls angemeldet und eingeschaltet, sonst spools.json. Ohne force höchstens alle 2 Minuten. */
  filamentSync: (force: boolean) => invoke<SyncResult>("filament_sync", { force }),
  spoolUpdate: (update: { id: string; remainingGrams: number; hidden: number; note: string }) =>
    invoke<void>("spool_update", { update }),
  runSetResult: (id: string, result: RunResult, note?: string) =>
    invoke<void>("run_set_result", { id, result, note: note ?? null }),
  runDeduct: (id: string, deductions: { index: number; spoolId: string | null; grams: number }[]) =>
    invoke<void>("run_deduct", { id, deductions }),
  runUndo: (id: string) => invoke<void>("run_undo", { id }),
  runDelete: (id: string) => invoke<void>("run_delete", { id }),
  /** Auftragsordner nach gesliced Dateien durchsuchen; liefert Meldungen */
  orderScan: (id: string) => invoke<string[]>("order_scan", { id }),
};

/** Ereignis aus dem Kern: Dateien oder Spulen haben sich außerhalb der App geändert */
export const EXTERNAL = "pm:external";

export function parseRunFilaments(json: string): RunFilament[] {
  try {
    return JSON.parse(json) as RunFilament[];
  } catch {
    return [];
  }
}

export const fmtGrams = (g: number) => (g >= 10 ? `${Math.round(g)} g` : `${g.toFixed(1)} g`);

// ---------- Farben und passende Spulen ----------

/** Farbnamen, wie man sie in einen Farbwunsch schreibt → ungefähre Farbe */
const COLOR_WORDS: [RegExp, string][] = [
  [/schwarz|black/, "#161616"], [/wei(ß|ss)|white/, "#FFFFFF"], [/grau|gray|grey|anthrazit/, "#898989"],
  [/silber|silver/, "#C0C0C0"], [/rot|red/, "#E02020"], [/orange/, "#F98C36"], [/gelb|yellow/, "#FFE133"],
  [/gold/, "#D4AF37"], [/hellgr(ü|ue)n|lime/, "#9CDB43"], [/gr(ü|ue)n|green/, "#0ACC38"],
  [/t(ü|ue)rkis|cyan|petrol/, "#0D9488"], [/hellblau|himmelblau/, "#5EB8F5"], [/dunkelblau|navy/, "#0D2A6B"],
  [/blau|blue/, "#0078BF"], [/lila|violett|purple|lavendel/, "#A03CF7"], [/pink|rosa|magenta/, "#F06EAA"],
  [/braun|brown|holz/, "#7C4B00"], [/beige|creme|natur|sand|elfenbein/, "#E8D9B5"],
];
const MATERIALS = ["PLA", "PETG", "ABS", "ASA", "TPU", "PA", "PC", "PVA"];

function hexRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** Abstand zweier Farben (0 = gleich, ~765 = schwarz/weiß), gewichtet nach Wahrnehmung */
export function colorDistance(a: string, b: string) {
  const x = hexRgb(a), y = hexRgb(b);
  if (!x || !y) return 999;
  const r = (x[0] + y[0]) / 2;
  const [dr, dg, db] = [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
  return Math.sqrt((2 + r / 256) * dr * dr + 4 * dg * dg + (2 + (255 - r) / 256) * db * db) / 3;
}

/** Farbwunsch („PETG rot“, „#ff0000“) in Farbe und ggf. Material zerlegen */
export function parseColorWish(wish: string): { color: string | null; material: string | null } {
  const w = wish.toLowerCase();
  const hex = /#[0-9a-f]{6}/i.exec(wish)?.[0] ?? null;
  const color = hex ?? COLOR_WORDS.find(([re]) => re.test(w))?.[1] ?? null;
  const material = MATERIALS.find((m) => new RegExp(`\\b${m}`, "i").test(wish)) ?? null;
  return { color, material };
}

/** Spulen nach Eignung sortieren: gleiches Material, ähnliche Farbe, genug Rest */
export function rankSpools(spools: Spool[], want: { color: string | null; material: string | null; grams?: number }) {
  return spools
    .filter((s) => !s.hidden && !s.missing && s.remainingGrams > 0)
    .filter((s) => !want.material || s.material.toUpperCase().startsWith(want.material.toUpperCase()))
    .map((s) => ({
      spool: s,
      distance: want.color ? colorDistance(s.color, want.color) : 0,
      enough: want.grams == null || s.remainingGrams >= want.grams,
    }))
    .sort((a, b) => Number(b.enough) - Number(a.enough) || a.distance - b.distance || a.spool.remainingGrams - b.spool.remainingGrams);
}

export const spoolLabel = (s: Spool) =>
  [s.brand !== "Generic" ? s.brand : "", s.series || s.material, s.colorName].filter(Boolean).join(" ");

export function catalogSrc(dataDir: string, e: CatalogEntry) {
  return e.thumbName ? convertFileSrc(`${dataDir}/catalog/${e.id}/${e.thumbName}`) : e.coverUrl;
}

export function parseFilaments(json: string): Filament[] {
  try {
    return JSON.parse(json) as Filament[];
  } catch {
    return [];
  }
}

export function fmtDuration(sec: number | null) {
  if (!sec) return "";
  const h = Math.floor(sec / 3600);
  const m = Math.round((sec % 3600) / 60);
  return h ? `${h} h ${m} min` : `${m} min`;
}

export function fmtSize(bytes: number | null) {
  if (!bytes) return "";
  return bytes > 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.round(bytes / 1e3)} kB`;
}

/** Plattform-Links, für die ein Import mehr bringt als ein einfacher Link */
export const isModelPlatform = (url: string) => /(printables\.com|makerworld\.com|thingiverse\.com)/i.test(url);


/** Bild aus dem Auftragsordner für <img> */
export function fileSrc(folder: string, rel: string) {
  return convertFileSrc(`${folder}/${rel}`);
}

export function orderFolder(dataDir: string, number: number) {
  return `${dataDir}/orders/${String(number).padStart(4, "0")}`;
}

export const isModelFile = (name: string) => /\.(3mf|stl|step|stp|obj)$/i.test(name);
export const isUrl = (s: string) => /^https?:\/\/\S+$/i.test(s.trim());

export interface BambuStatus {
  region: string; accountName: string | null; uid: number | null; expiresAt: string | null;
  filament: boolean; printers: boolean; makerworld: boolean; filamentWrite: boolean;
  connected: boolean; expired: boolean; lastOkAt: string | null; lastError: string | null; loggingIn: boolean;
  filamentSyncedAt: string | null; mwSyncedAt: string | null;
}
