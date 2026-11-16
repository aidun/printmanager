import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import {
  BarChart3, Boxes, CalendarCheck, Kanban, Layers, List, Plus, Printer, Search, Settings as SettingsIcon, Users, Box,
  type LucideIcon,
} from "lucide-react";
import { api, CHANGED, EXTERNAL, type AppInfo, type BambuStatus, type OrderSummary, type PrinterView } from "./api";
import { daysUntil } from "./status";
import { Board } from "./Board";
import { ListView } from "./ListView";
import { OrderPanel } from "./OrderPanel";
import { NewOrderWizard } from "./NewOrderWizard";
import { CatalogView } from "./CatalogView";
import { FilamentView } from "./FilamentView";
import { PeopleView } from "./PeopleView";
import { SettingsView } from "./SettingsView";
import { TodayView } from "./TodayView";
import { StatsView } from "./StatsView";
import { PrintersView } from "./PrintersView";

type View = "today" | "board" | "list" | "catalog" | "filament" | "printers" | "stats" | "people" | "settings";

export function App() {
  const [view, setView] = useState<View>("today");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [orders, setOrders] = useState<OrderSummary[]>([]);
  const [search, setSearch] = useState("");
  const [openId, setOpenId] = useState<string | null>(null);
  const [newOrder, setNewOrder] = useState<{ paths: string[] } | null>(null);
  const [dragging, setDragging] = useState(false);
  const [toast, setToast] = useState<{ text: string; error?: boolean } | null>(null);
  const [rev, setRev] = useState(0); // erhöht sich nach jeder Änderung, damit offene Ansichten neu laden
  const [printers, setPrinters] = useState<PrinterView[]>([]);
  const [account, setAccount] = useState<BambuStatus | null>(null);

  const openIdRef = useRef(openId);
  openIdRef.current = openId;
  const viewRef = useRef(view);
  viewRef.current = view;
  const newOrderRef = useRef(newOrder);
  newOrderRef.current = newOrder;
  /** Der Assistent für neue Aufträge meldet sich hier für Drag&Drop an */
  const wizardDrop = useRef<((paths: string[]) => void) | null>(null);

  const notify = useCallback((text: string, error = false) => {
    setToast({ text, error });
    window.setTimeout(() => setToast((t) => (t?.text === text ? null : t)), error ? 6000 : 2500);
  }, []);

  const fail = useCallback((e: unknown) => notify(String(e), true), [notify]);

  const reload = useCallback(async () => {
    try {
      setOrders(await api.orders(search));
      setRev((r) => r + 1);
    } catch (e) {
      fail(e);
    }
  }, [search, fail]);

  useEffect(() => {
    api.appInfo().then(setInfo).catch(fail);
  }, [fail]);

  // Fußzeile der Seitenleiste: Drucker und Bambu-Konto
  useEffect(() => {
    const load = () => {
      api.printers().then((r) => setPrinters(r.connected && r.enabled ? r.printers : [])).catch(() => setPrinters([]));
      api.bambuStatus().then(setAccount).catch(() => setAccount(null));
    };
    load();
    const t = window.setInterval(load, 20000);
    return () => window.clearInterval(t);
  }, [rev]);

  // Automatisch aktualisieren: nach jeder Änderung, wenn das Fenster wieder nach vorn kommt
  // und alle 20 Sekunden (z.B. für Änderungen von einem zweiten Rechner oder im Auftragsordner)
  const reloadRef = useRef(reload);
  reloadRef.current = reload;
  useEffect(() => {
    let timer = 0;
    const soon = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => reloadRef.current(), 100);
    };
    const onVisible = () => document.visibilityState === "visible" && soon();
    window.addEventListener(CHANGED, soon);
    window.addEventListener("focus", soon);
    document.addEventListener("visibilitychange", onVisible);
    const poll = window.setInterval(() => document.visibilityState === "visible" && reloadRef.current(), 20000);
    return () => {
      window.removeEventListener(CHANGED, soon);
      window.removeEventListener("focus", soon);
      document.removeEventListener("visibilitychange", onVisible);
      window.clearInterval(poll);
      window.clearTimeout(timer);
    };
  }, []);

  // Änderungen außerhalb der App (gesliced Datei im Auftragsordner, Spulen in Bambu Studio)
  useEffect(() => {
    const un = listen<{ messages: string[] }>(EXTERNAL, (ev) => {
      window.dispatchEvent(new Event(CHANGED));
      if (ev.payload.messages.length) notify(ev.payload.messages.join(" · "));
    });
    return () => {
      un.then((f) => f());
    };
  }, [notify]);

  useEffect(() => {
    const t = window.setTimeout(reload, 150);
    return () => window.clearTimeout(t);
  }, [reload]);

  // Dateien ins Fenster ziehen: an den offenen Auftrag hängen oder einen neuen Auftrag anlegen
  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent(async (ev) => {
      const p = ev.payload;
      if (p.type === "enter" || p.type === "over") setDragging(true);
      else if (p.type === "leave") setDragging(false);
      else if (p.type === "drop") {
        setDragging(false);
        if (!p.paths.length) return;
        const target = openIdRef.current;
        if (newOrderRef.current && wizardDrop.current) {
          wizardDrop.current(p.paths);
        } else if (viewRef.current === "catalog" && !target) {
          try {
            const ids = await api.catalogImportPaths(p.paths);
            notify(ids.length === 1 ? "Projekt zum Katalog hinzugefügt" : `${ids.length} Projekte zum Katalog hinzugefügt`);
            setRev((r) => r + 1);
          } catch (e) {
            fail(e);
          }
        } else if (target) {
          try {
            const n = await api.addPaths(target, p.paths);
            notify(n === 1 ? "Datei angehängt" : `${n} Dateien angehängt`);
            reload();
          } catch (e) {
            fail(e);
          }
        } else {
          setNewOrder({ paths: p.paths });
        }
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [notify, fail, reload]);

  // Tastenkürzel: Cmd/Strg+N neuer Auftrag, Cmd/Strg+F Suche, Esc schließt das Detail
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setNewOrder({ paths: [] });
      } else if (mod && e.key.toLowerCase() === "f") {
        e.preventDefault();
        document.getElementById("search")?.focus();
      } else if (e.key === "Escape" && !document.querySelector("dialog[open]")) {
        setOpenId(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const open = orders.filter((o) => !o.archivedAt && !["uebergeben", "abgelehnt"].includes(o.status));
  const due = open.filter((o) => o.status !== "fertig" && o.status !== "pausiert" && (daysUntil(o.dueDate) ?? 99) <= 0).length;
  const printing = printers.filter((p) => p.state === "printing" || p.state === "paused");
  const nav: { section?: string; id: View; label: string; icon: LucideIcon; count?: number; alert?: boolean; live?: boolean }[] = [
    { id: "today", label: "Heute", icon: CalendarCheck, count: due || undefined, alert: due > 0 },
    { id: "board", label: "Board", icon: Kanban, count: open.length || undefined },
    { id: "list", label: "Alle Aufträge", icon: List },
    { section: "Bibliothek", id: "catalog", label: "Katalog", icon: Boxes },
    { id: "filament", label: "Filament", icon: Layers },
    { section: "Werkstatt", id: "printers", label: "Drucker", icon: Printer, live: printing.length > 0 },
    { id: "stats", label: "Statistik", icon: BarChart3 },
    { section: "Verwaltung", id: "people", label: "Personen", icon: Users },
    { id: "settings", label: "Einstellungen", icon: SettingsIcon },
  ];
  /** Druckdatei direkt in Bambu Studio öffnen (rückt den Auftrag ggf. auf „In Studio“ vor) */
  const openInStudio = useCallback(
    async (attachmentId: string) => {
      try {
        const advanced = await api.openInStudio(attachmentId);
        notify(advanced ? "Bambu Studio wird geöffnet · Status jetzt „In Studio“" : "Bambu Studio wird geöffnet");
        reload();
      } catch (e) {
        fail(e);
      }
    },
    [notify, fail, reload],
  );

  const ctx = { info, orders, reload, notify, fail, open: setOpenId, openId, openInStudio };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="logo" aria-hidden><Box size={14} strokeWidth={2.4} /></span> Printmanager
        </div>
        <button className="primary new" onClick={() => setNewOrder({ paths: [] })} title="Neuer Auftrag (⌘N)">
          <Plus size={15} strokeWidth={2.4} /> Neuer Auftrag <span className="kbd">⌘N</span>
        </button>
        <nav>
          {nav.map((n) => (
            <div key={n.id} style={{ display: "contents" }}>
              {n.section && <div className="nav-section">{n.section}</div>}
              <button className={view === n.id ? "active" : ""} onClick={() => setView(n.id)} aria-current={view === n.id ? "page" : undefined}>
                <span className="icon" aria-hidden><n.icon size={16} strokeWidth={1.9} /></span>
                {n.label}
                {n.live ? <span className="nav-live" title="Druckt gerade" /> : n.count ? <span className={`nav-count ${n.alert ? "alert" : ""}`}>{n.count}</span> : null}
              </button>
            </div>
          ))}
        </nav>
        <div className="sidebar-foot">
          {printing.map((p) => (
            <button key={p.id} className="link foot-row" onClick={() => setView("printers")} title={p.taskName ?? ""} style={{ color: "inherit" }}>
              <Printer size={13} />
              <span className="grow">{p.name} · {p.progress ?? 0} %</span>
              <span className="mini-bar"><span style={{ width: `${p.progress ?? 0}%` }} /></span>
            </button>
          ))}
          <div className="foot-row">
            <span className="online-dot" style={{ background: account?.connected ? "var(--s-druckt)" : account?.expired ? "var(--late)" : undefined }} />
            <span className="grow">
              {account?.connected ? `Bambu-Konto${account.accountName ? ` · ${account.accountName}` : ""}` : account?.expired ? "Bambu-Anmeldung abgelaufen" : "Bambu-Konto nicht verbunden"}
            </span>
          </div>
          <div className="foot-row">{open.length} offene Aufträge</div>
        </div>
      </aside>

      <main className="main">
        {(view === "board" || view === "list") && (
          <header className="toolbar">
            <h1>{view === "board" ? "Board" : "Alle Aufträge"}</h1>
            <span className="page-sub">{view === "board" ? "Aufträge nach Arbeitsschritt – Karten ziehen, um den Status zu ändern" : "Suchen, filtern und sortieren"}</span>
            <label className="search">
              <Search size={14} />
              <input
                id="search"
                type="search"
                placeholder="Suchen: Titel, Person, Nr. …  ⌘F"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </label>
          </header>
        )}
        {view === "today" && <TodayView {...ctx} rev={rev} onShowFilament={() => setView("filament")} />}
        {view === "board" && <Board {...ctx} />}
        {view === "list" && <ListView {...ctx} />}
        {view === "catalog" && info && <CatalogView dataDir={info.dataDir} rev={rev} notify={notify} fail={fail} />}
        {view === "filament" && info && <FilamentView info={info} rev={rev} notify={notify} fail={fail} />}
        {view === "printers" && <PrintersView rev={rev} open={setOpenId} fail={fail} onSettings={() => setView("settings")} />}
        {view === "stats" && <StatsView orders={orders} rev={rev} fail={fail} />}
        {view === "people" && <PeopleView {...ctx} rev={rev} onShowOrders={(name) => { setSearch(name); setView("list"); }} />}
        {view === "settings" && info && <SettingsView info={info} onSaved={(i) => { setInfo(i); reload(); }} notify={notify} fail={fail} />}
      </main>

      {openId && info && (
        <OrderPanel
          key={openId}
          id={openId}
          info={info}
          rev={rev}
          onClose={() => setOpenId(null)}
          onChanged={reload}
          openInStudio={openInStudio}
          notify={notify}
          fail={fail}
        />
      )}

      {newOrder && info && (
        <NewOrderWizard
          dataDir={info.dataDir}
          dropRef={wizardDrop}
          paths={newOrder.paths}
          onClose={() => setNewOrder(null)}
          onCreated={(id) => {
            setNewOrder(null);
            reload();
            setOpenId(id);
          }}
          fail={fail}
        />
      )}

      {dragging && (
        <div className="drop-overlay">
          <div>
            {newOrder
              ? "Loslassen, um die Dateien zum neuen Auftrag zu nehmen"
              : openId
                ? `Loslassen, um die Dateien an Auftrag #${orders.find((o) => o.id === openId)?.number ?? ""} zu hängen`
                : view === "catalog"
                  ? "Loslassen, um die Dateien in den Katalog zu übernehmen"
                  : "Loslassen, um einen neuen Auftrag mit diesen Dateien anzulegen"}
          </div>
        </div>
      )}

      {toast && <div className={`toast ${toast.error ? "error" : ""}`}>{toast.text}</div>}
    </div>
  );
}

export interface ViewCtx {
  info: AppInfo | null;
  orders: OrderSummary[];
  reload: () => void;
  notify: (t: string, error?: boolean) => void;
  fail: (e: unknown) => void;
  open: (id: string) => void;
  openId: string | null;
  openInStudio: (attachmentId: string) => void;
}
