import { createPinia } from "pinia";
import { createApp } from "vue";
import App from "../src/App.vue";
import { useAppStore } from "../src/stores/app";
import "./preview.css";

createApp(App).use(createPinia()).mount("#app");

// `?view=task&task=1` renders a specific screen for review.
const params = new URLSearchParams(location.search);
const view = params.get("view");
if (view) {
  setTimeout(() => {
    const store = useAppStore();
    store.view = view as never;
    store.selectedTaskId = Number(params.get("task") ?? 1);
  }, 150);
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
