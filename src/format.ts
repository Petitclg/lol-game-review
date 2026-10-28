const QUEUE_LABELS: Record<number, string> = {
  420: "Ranked Solo",
  440: "Ranked Flex",
  400: "Normal Draft",
  430: "Normal Blind",
  450: "ARAM",
  490: "Quickplay",
};

export function queueLabel(queueId: number): string {
  return QUEUE_LABELS[queueId] ?? `File ${queueId}`;
}

/** Durée en secondes → "mm:ss" (ou "h:mm:ss" au-delà d'une heure). */
export function formatDuration(totalSecs: number): string {
  const secs = Math.max(0, Math.floor(Number.isFinite(totalSecs) ? totalSecs : 0));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const ss = String(s).padStart(2, "0");
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${ss}`;
  return `${String(m).padStart(2, "0")}:${ss}`;
}

export function formatRelativeDate(thenMs: number, nowMs: number = Date.now()): string {
  const diff = Math.max(0, nowMs - thenMs);
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "À l'instant";
  if (minutes < 60) return `Il y a ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Il y a ${hours} h`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "Hier";
  if (days < 30) return `Il y a ${days} j`;
  const months = Math.floor(days / 30);
  if (months < 12) return `Il y a ${months} mois`;
  const years = Math.floor(days / 365);
  return `Il y a ${Math.max(1, years)} an${years > 1 ? "s" : ""}`;
}

export function formatKda(kills: number, deaths: number, assists: number): string {
  return `${kills}/${deaths}/${assists}`;
}
