import type { View } from "../stores/app";

export interface Shortcut {
  /** How it is written in the interface. */
  combo: string;
  label: string;
  /** The `event.key` it matches, lower-cased. */
  key: string;
  /** True when the command key must be held. */
  meta: boolean;
}

/**
 * The keys the app answers to, in the order they are listed in Settings.
 * `view` shortcuts navigate; the rest are handled by name.
 */
export const SHORTCUTS: Array<Shortcut & { action: View | "newTask" | "hide" }> = [
  { combo: "⌘1", label: "Today", key: "1", meta: true, action: "today" },
  { combo: "⌘2", label: "Goal", key: "2", meta: true, action: "goal" },
  { combo: "⌘3", label: "Changes", key: "3", meta: true, action: "changes" },
  { combo: "⌘4", label: "Settings", key: "4", meta: true, action: "settings" },
  { combo: "⌘N", label: "New task", key: "n", meta: true, action: "newTask" },
  { combo: "⌘G", label: "New goal", key: "g", meta: true, action: "goal" },
  { combo: "⌘W", label: "Hide", key: "w", meta: true, action: "hide" },
];

/** Typing in a field takes precedence over every shortcut but Escape. */
export function isTyping(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  const tag = element.tagName?.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || element.isContentEditable;
}

/** The shortcut this event triggers, if any. */
export function match(event: KeyboardEvent) {
  if (!event.metaKey || event.ctrlKey || event.altKey) return undefined;
  const key = event.key.toLowerCase();
  return SHORTCUTS.find((shortcut) => shortcut.key === key && shortcut.meta);
}
