/**
 * Puts text on the clipboard, and says whether it got there.
 *
 * The webview may refuse — the clipboard is a permission like any other — so
 * this never throws and never pretends. A caller that shows the text as well
 * can leave it on screen for the developer to copy by hand.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
