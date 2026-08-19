/**
 * The small sounds the interface makes while someone is looking at it.
 * Synthesised rather than bundled: a couple of oscillators weigh nothing and
 * need no asset past the content policy.
 *
 * The alarm that ends a session is not here. That one has to sound when every
 * window is hidden, which is when a webview can be suspended, so the backend
 * plays it through the system instead.
 */

let context: AudioContext | undefined;

/** Created on the first sound, because a page may never make one. */
function audio(): AudioContext | undefined {
  try {
    context ??= new AudioContext();
    // A context can be suspended by the browser until something asks for it.
    if (context.state === "suspended") void context.resume();
    return context;
  } catch {
    return undefined;
  }
}

interface Note {
  /** Hertz. */
  pitch: number;
  /** Seconds from the start of the sequence. */
  at: number;
  /** Seconds the note rings for. */
  length: number;
  gain?: number;
}

function play(notes: Note[]) {
  const ctx = audio();
  if (!ctx) return;
  const now = ctx.currentTime;
  for (const note of notes) {
    const oscillator = ctx.createOscillator();
    const envelope = ctx.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = note.pitch;

    // A hard start and stop clicks; ramping in and out is what makes it read
    // as a chime rather than a beep.
    const start = now + note.at;
    const peak = note.gain ?? 0.18;
    envelope.gain.setValueAtTime(0.0001, start);
    envelope.gain.exponentialRampToValueAtTime(peak, start + 0.012);
    envelope.gain.exponentialRampToValueAtTime(0.0001, start + note.length);

    oscillator.connect(envelope).connect(ctx.destination);
    oscillator.start(start);
    oscillator.stop(start + note.length + 0.02);
  }
}

/** A task crossing the line: quieter, two notes, over in a moment. */
export function chime() {
  play([
    { pitch: 987.8, at: 0, length: 0.14 },
    { pitch: 1318.5, at: 0.11, length: 0.26 },
  ]);
}
