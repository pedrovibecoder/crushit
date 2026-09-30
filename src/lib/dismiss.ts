import { onBeforeUnmount, onMounted, type Ref } from "vue";

/**
 * Closes a panel when the next press lands anywhere else.
 *
 * On `pointerdown` rather than `click`, so a panel gets out of the way as the
 * press begins rather than after it finishes. A control that opens a panel
 * should stop the event on its own `pointerdown` — otherwise this closes the
 * panel and the control's click immediately reopens it, and the panel appears
 * never to close at all.
 */
export function onPressOutside(target: Ref<HTMLElement | null>, dismiss: () => void) {
  function onPointerDown(event: PointerEvent) {
    const element = target.value;
    if (!element) return;
    if (event.target instanceof Node && element.contains(event.target)) return;
    dismiss();
  }

  onMounted(() => document.addEventListener("pointerdown", onPointerDown));
  onBeforeUnmount(() => document.removeEventListener("pointerdown", onPointerDown));
}
