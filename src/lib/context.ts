import type { Task } from "../types";

/** What the caller knows that a task does not carry on its own. */
export interface ContextNames {
  project: string | null;
  branch: string | null;
  /** Human labels for a category slug and a tag slug. */
  categoryLabel: (slug: string) => string;
  tagLabel: (slug: string) => string;
}

/**
 * The selected tasks written out for an agent to read.
 *
 * Plain text rather than JSON: this is pasted into a chat, and prose with
 * headings is what a model reads best — and what a person can check before
 * sending. Everything already recorded about a task goes in, because the whole
 * point is not having to retype what the app already knows.
 */
export function buildContext(tasks: Task[], names: ContextNames): string {
  const lines: string[] = [];

  const where = [names.project, names.branch].filter(Boolean).join(" · ");
  lines.push(where ? `# Context: ${where}` : "# Context");
  lines.push("");
  lines.push(
    tasks.length === 1
      ? "One task, as recorded in the tracker:"
      : `${tasks.length} tasks, as recorded in the tracker:`,
  );
  lines.push("");

  tasks.forEach((task, index) => {
    lines.push(`## ${index + 1}. ${task.title}`);

    const facts = [`Status: ${task.status}`, `Type: ${names.categoryLabel(task.category)}`];
    if (task.tags.length) facts.push(`Tags: ${task.tags.map(names.tagLabel).join(", ")}`);
    if (task.storyPoints) facts.push(`Points: ${task.storyPoints}`);
    if (task.estimateMinutes) facts.push(`Estimate: ${task.estimateMinutes}m`);
    facts.push(`Planned: ${task.plannedFor ?? "not scheduled"}`);
    lines.push(facts.join(" · "));

    if (task.description?.trim()) {
      lines.push("");
      lines.push(task.description.trim());
    }

    if (task.criteria.length) {
      lines.push("");
      lines.push("Acceptance criteria:");
      for (const criterion of task.criteria) {
        lines.push(`- [${criterion.isMet ? "x" : " "}] ${criterion.text}`);
      }
    }

    if (task.files.length) {
      lines.push("");
      lines.push("Relevant files:");
      for (const path of task.files) lines.push(`- ${path}`);
    }

    // Named rather than numbered: an id means nothing outside this database.
    const blockers = task.dependsOn
      .map((id) => tasks.find((other) => other.id === id)?.title)
      .filter((title): title is string => !!title);
    if (blockers.length) {
      lines.push("");
      lines.push(`Depends on: ${blockers.join("; ")}`);
    }

    lines.push("");
  });

  return lines.join("\n").trimEnd() + "\n";
}
