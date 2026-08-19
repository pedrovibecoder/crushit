import { createPinia } from "pinia";
import { createApp } from "vue";
import App from "../src/App.vue";
import { useAppStore } from "../src/stores/app";
import "./preview.css";

const previewApp = createApp(App).use(createPinia());
/**
 * A component that throws during render unmounts silently, leaving a blank
 * panel and no clue why. In review that is the difference between finding a
 * bug in seconds and mistaking it for a rendering artifact.
 */
previewApp.config.errorHandler = (error, _instance, info) => {
  const box = document.createElement("div");
  box.textContent = `RENDER ERROR (${info}): ${String(error)}\n${(error as Error)?.stack ?? ""}`;
  box.style.cssText =
    "position:fixed;inset:0;z-index:99999;background:#ff0;color:#000;font:11px monospace;padding:8px;white-space:pre-wrap;overflow:auto";
  document.body.appendChild(box);
};
previewApp.mount("#app");

// `?view=task&task=1` renders a specific screen for review.
const params = new URLSearchParams(location.search);
const view = params.get("view");
if (view) {
  setTimeout(() => {
    const store = useAppStore();
    // The task has to be chosen before the screen is: the app sends you back to
    // Today if it lands on the task screen with nothing selected.
    store.selectedTaskId = Number(params.get("task") ?? 1);
    store.view = view as never;
  }, 500);
}

// `?celebrate=1` shows the task-completed celebration.
if (params.get("celebrate")) {
  setTimeout(async () => {
    const { useTasksStore } = await import("../src/stores/tasks");
    useTasksStore().celebration = { title: "Create invoice PDF service", at: Date.now() };
  }, 260);
}

// `?type=1` fills the quick-add input so its category chips appear.
if (params.get("type")) {
  setTimeout(() => {
    const field = document.querySelector<HTMLInputElement>(
      'input[aria-label="New task title"]',
    );
    if (!field) return;
    field.value = "Add invoice export";
    field.dispatchEvent(new Event("input", { bubbles: true }));
  }, 300);
}

// `?open=model` expands the model dropdown so it can be reviewed open.
if (params.get("open") === "model") {
  setTimeout(() => {
    const button = [...document.querySelectorAll("button")].find(
      (element) => element.getAttribute("aria-label") === "Model",
    );
    button?.click();
    setTimeout(() => button?.scrollIntoView({ block: "center" }), 120);
  }, 400);
}

// `?press=Settings` clicks the button with that title, which is how a screen
// reached by a button rather than by `?view=` gets opened for review.
if (params.get("press")) {
  setTimeout(() => {
    const wanted = params.get("press")!;
    const button = [...document.querySelectorAll("button")].find(
      (element) => element.title === wanted || element.getAttribute("aria-label") === wanted,
    );
    button?.click();
  }, 500);
}

// `?reveal=Distractions` scrolls a section of a long screen into view, so a
// part that is normally below the fold can be reviewed.
if (params.get("reveal")) {
  setTimeout(() => {
    const wanted = params.get("reveal")!.toLowerCase();
    const heading = [...document.querySelectorAll("h2, h1, p")].find((element) =>
      element.textContent?.trim().toLowerCase().startsWith(wanted),
    );
    heading?.scrollIntoView({ block: "start" });
    // After `?view=`, which swaps the screen half a second in.
  }, 900);
}

// `?panel=calendar` or `?panel=project` opens one of Today's header panels.
const PANELS: Record<string, string> = {
  calendar: "Pick a day",
  project: "Switch project",
};
if (PANELS[params.get("panel") ?? ""]) {
  setTimeout(() => {
    const title = PANELS[params.get("panel") ?? ""];
    const button = [...document.querySelectorAll("button")].find(
      (element) => element.title === title,
    );
    button?.click();
  }, 340);
}

// `?compose=1` opens the inline add-task form for review.
if (params.get("compose")) {
  setTimeout(() => {
    const button = [...document.querySelectorAll("button")].find((element) =>
      element.textContent?.trim().startsWith("Task"),
    );
    button?.click();
  }, 320);
}

// Preview only: expose the panel's rendered height for measurement.
setTimeout(() => {
  const panel = document.querySelector("#app > div") as HTMLElement | null;
  document.documentElement.setAttribute(
    "data-shell-height",
    String(panel ? Math.ceil(panel.getBoundingClientRect().height) : 0),
  );
}, 900);

// `?diff=1` opens the first changed file's diff.
if (params.get("diff")) {
  setTimeout(async () => {
    const { useReviewStore } = await import("../src/stores/review");
    void useReviewStore().openDiff("src/permissions/keys.ts");
  }, 400);
}

// `?confirm=delete` opens a task and arms its delete confirmation.
if (params.get("confirm") === "delete") {
  const find = (label: string) =>
    [...document.querySelectorAll("button")].find(
      (element) => element.getAttribute("aria-label") === label,
    );
  const tick = () => new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  void (async () => {
    for (let i = 0; i < 300; i += 1) {
      const row = document.querySelector<HTMLElement>('div[role="button"]');
      if (row) { row.click(); break; }
      await tick();
    }
    for (let i = 0; i < 300; i += 1) {
      const button = find("Delete task");
      if (button) { button.click(); break; }
      await tick();
    }
    // `?confirm=delete&go=1` follows through, to watch the row leave.
    if (params.get("go")) {
      for (let i = 0; i < 300; i += 1) {
        const button = [...document.querySelectorAll("button")].find(
          (element) => element.textContent?.trim() === "Delete",
        );
        if (button) { button.click(); break; }
        await tick();
      }
    }
  })();
}

// `?rowexit=<ms>` puts the delete animation on a row and holds it at that
// moment, without deleting anything — the animation on its own.
if (params.get("rowexit")) {
  setTimeout(() => {
    const style = document.createElement("style");
    style.textContent = `.row-deleting { animation-play-state: paused !important; animation-delay: -${params.get("rowexit")}ms !important; }`;
    document.head.appendChild(style);
    document.querySelector('div[role="button"]')?.classList.add("row-deleting");
  }, 500);
}

// `?slackview=list|reply` opens the Slack screen, and the first conversation
// with a drafted reply in it.
if (params.get("slackview")) {
  setTimeout(async () => {
    const { useSlackStore } = await import("../src/stores/slack");
    const store = useAppStore();
    const slack = useSlackStore();
    store.view = "slack";
    await slack.refreshAccount();
    await slack.load();
    if (params.get("slackview") === "reply") {
      slack.openConversation("D1");
      await slack.writeDraft();
    }
  }, 900);
}

// `?day=-1` steps the day back, for reviewing what carries over.
if (params.get("day")) {
  const steps = Number(params.get("day"));
  const label = steps < 0 ? "Previous day" : "Next day";
  const tick = () => new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  void (async () => {
    for (let i = 0; i < Math.abs(steps); i += 1) {
      for (let attempt = 0; attempt < 300; attempt += 1) {
        const button = [...document.querySelectorAll("button")].find(
          (element) => element.getAttribute("aria-label") === label,
        );
        if (button) { button.click(); break; }
        await tick();
      }
      await tick();
    }
  })();
}

// `?settings=<section>` opens Settings and scrolls a section into view.
if (params.get("settings")) {
  setTimeout(() => {
    useAppStore().view = "settings";
    const wanted = params.get("settings")?.toLowerCase();
    setTimeout(() => {
      const heading = [...document.querySelectorAll("h2")].find(
        (element) => element.textContent?.trim().toLowerCase() === wanted,
      );
      heading?.scrollIntoView({ block: "start" });
    }, 120);
  }, 900);
}
