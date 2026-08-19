/** `24:31`, or `1:04:12` once past an hour — matches the menu-bar clock. */
export function formatClock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(secs)}` : `${minutes}:${pad(secs)}`;
}

/** Compact duration for list rows: `30m`, `1h 15m`. */
export function formatDuration(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours}h` : `${hours}h ${rest}m`;
}

/**
 * A span of time in the units a person would say it in: `1h 15m`, `45m`,
 * `30s`. Used for totals, where a `mm:ss` clock stops being readable.
 */
export function formatSpan(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  if (total < 60) return `${total}s`;
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours === 0) return `${minutes}m`;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

/** Shortens a path for display: `~/projects/my-app`. */
export function shortenPath(path: string, home = "/Users"): string {
  const parts = path.split("/");
  if (path.startsWith(home) && parts.length > 3) {
    return `~/${parts.slice(3).join("/")}`;
  }
  return path;
}

/** Today on the local clock, as `YYYY-MM-DD` — the form tasks are filed under. */
export function localDay(date = new Date()): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/** The same day, moved by a number of days. */
export function shiftDay(day: string, by: number): string {
  const date = new Date(`${day}T12:00:00`);
  date.setDate(date.getDate() + by);
  return localDay(date);
}

/**
 * How a person would name the day: `Today`, `Yesterday`, `Tomorrow`, and a
 * date for anything further out.
 */
export function dayName(day: string, today = localDay()): string {
  const days = Math.round(
    (new Date(`${day}T12:00:00`).getTime() - new Date(`${today}T12:00:00`).getTime()) / 86_400_000,
  );
  if (days === 0) return "Today";
  if (days === -1) return "Yesterday";
  if (days === 1) return "Tomorrow";
  return new Date(`${day}T12:00:00`).toLocaleDateString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}
