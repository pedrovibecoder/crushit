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

/** Shortens a path for display: `~/projects/my-app`. */
export function shortenPath(path: string, home = "/Users"): string {
  const parts = path.split("/");
  if (path.startsWith(home) && parts.length > 3) {
    return `~/${parts.slice(3).join("/")}`;
  }
  return path;
}
