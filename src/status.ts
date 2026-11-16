import type { Status } from "./api";

export const STATUS: Record<Status, { label: string; hint: string; color: string }> = {
  eingang: { label: "Eingang", hint: "Neu, noch nicht angeschaut", color: "var(--s-eingang)" },
  klaeren: { label: "Klären", hint: "Rückfragen offen (Farbe, Größe, Datei …)", color: "var(--s-klaeren)" },
  vorbereiten: { label: "Vorbereiten", hint: "Modell suchen oder anpassen", color: "var(--s-vorbereiten)" },
  in_studio: { label: "In Studio", hint: "Wird in Bambu Studio eingerichtet", color: "var(--s-studio)" },
  druckt: { label: "Druckt", hint: "Liegt auf dem Drucker", color: "var(--s-druckt)" },
  nacharbeit: { label: "Nacharbeit", hint: "Stützen entfernen, schleifen, kleben", color: "var(--s-nacharbeit)" },
  fertig: { label: "Fertig", hint: "Bereit zur Übergabe", color: "var(--s-fertig)" },
  uebergeben: { label: "Übergeben", hint: "Abgeholt bzw. verschickt", color: "var(--s-uebergeben)" },
  abgelehnt: { label: "Abgelehnt", hint: "Wird nicht gedruckt", color: "var(--s-abgelehnt)" },
  pausiert: { label: "Pausiert", hint: "Liegt vorerst auf Eis", color: "var(--s-pausiert)" },
};

export const BOARD_STATUSES: Status[] = [
  "eingang", "klaeren", "vorbereiten", "in_studio", "druckt", "nacharbeit", "fertig", "uebergeben",
];
export const SIDE_STATUSES: Status[] = ["pausiert", "abgelehnt"];
export const ALL_STATUSES: Status[] = [...BOARD_STATUSES, ...SIDE_STATUSES];

export const PRIORITY = ["normal", "hoch", "dringend"] as const;

export function fmtDate(iso: string | null | undefined) {
  if (!iso) return "";
  const d = new Date(iso.length === 10 ? iso + "T00:00:00" : iso);
  return d.toLocaleDateString("de-DE", { day: "2-digit", month: "2-digit", year: "numeric" });
}

export function fmtDateTime(iso: string) {
  return new Date(iso).toLocaleString("de-DE", {
    day: "2-digit", month: "2-digit", year: "numeric", hour: "2-digit", minute: "2-digit",
  });
}

/** Tage bis zum Fälligkeitsdatum (negativ = überfällig) */
export function daysUntil(date: string | null) {
  if (!date) return null;
  const due = new Date(date + "T00:00:00").getTime();
  const today = new Date(new Date().toDateString()).getTime();
  return Math.round((due - today) / 86400000);
}
