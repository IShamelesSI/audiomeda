// The only module that talks to the engine. Components read `engine` and call `actions`;
// they never invoke Tauri directly, so a new skin can't break the plumbing.

import { invoke } from "@tauri-apps/api/core";
import type { Label, Request, Response, Snapshot } from "./types";

export const engine = $state({
  snap: null as Snapshot | null,
  /** Set while the engine can't be reached. */
  error: null as string | null,
  toasts: [] as { id: number; text: string }[],
  /** exe path -> icon data URL (null = has none). Filled by appIcon(). */
  icons: {} as Record<string, string | null>,
});

// plain Set, not $state: this is called while rendering, where writing state is forbidden
const iconRequested = new Set<string>();

/** The app's real icon; fetched once per exe and cached. Null until it arrives. */
export function appIcon(path: string | undefined): string | null {
  if (!path) return null;
  if (!iconRequested.has(path)) {
    iconRequested.add(path);
    invoke<string | null>("app_icon", { path })
      .then((url) => (engine.icons[path] = url))
      .catch(() => {});
  }
  return engine.icons[path] ?? null;
}

/** Faders show position; the engine stores gain. Same curve as the CLI (protocol/src/state.rs). */
export const gainFromPosition = (p: number) => p * p;
export const positionFromGain = (g: number) => Math.sqrt(Math.max(0, g));

let toastId = 0;
export function toast(text: string) {
  const id = ++toastId;
  engine.toasts.push({ id, text });
  setTimeout(() => (engine.toasts = engine.toasts.filter((t) => t.id !== id)), 4000);
}

async function call(req: Request): Promise<Response> {
  return invoke<Response>("call", { req });
}

async function refresh() {
  try {
    const r = await call({ op: "snapshot" });
    if (r.type === "snapshot") {
      const { type: _, ...snap } = r;
      engine.snap = snap;
      engine.error = null;
    }
  } catch (e) {
    engine.error = String(e);
  }
}

/** Polls the engine while the window is open. shortcut: polling until the engine pushes changes. */
export function startPolling(everyMs = 300): () => void {
  refresh();
  const t = setInterval(refresh, everyMs);
  return () => clearInterval(t);
}

// While a fader is dragged, only the newest value per target matters: one request in
// flight per target, the latest waiting value sent when it returns.
const inFlight = new Set<string>();
const waiting = new Map<string, Request>();

function send(target: string, req: Request) {
  if (inFlight.has(target)) {
    waiting.set(target, req);
    return;
  }
  inFlight.add(target);
  call(req)
    .then((r) => r.type === "error" && toast(r.message))
    .catch((e) => (engine.error = String(e)))
    .finally(() => {
      inFlight.delete(target);
      const next = waiting.get(target);
      waiting.delete(target);
      if (next) send(target, next);
      else refresh();
    });
}

export const actions = {
  setInputVolume: (key: string, gain: number) => send(`in:${key}`, { op: "set_input", key, volume: gain }),
  setInputMute: (key: string, mute: boolean) => send(`in-mute:${key}`, { op: "set_input", key, mute }),
  setOutputVolume: (id: string, gain: number) => send(`out:${id}`, { op: "set_output", id, volume: gain }),
  setOutputMute: (id: string, mute: boolean) => send(`out-mute:${id}`, { op: "set_output", id, mute }),
  setCellVolume: (input: string, output: string, gain: number) =>
    send(`cell:${input}:${output}`, { op: "set_cell", input, output, volume: gain, on: true }),
  setCellOn: (input: string, output: string, on: boolean) =>
    send(`cell-on:${input}:${output}`, { op: "set_cell", input, output, on }),
  resetInput: (key: string) => send(`reset:${key}`, { op: "reset_input", key }),
  addOutput: (id: string) => send(`add-out:${id}`, { op: "add_output", id }),
  removeOutput: (id: string) => send(`rm-out:${id}`, { op: "remove_output", id }),
  addMic: (id: string) => send(`add-mic:${id}`, { op: "add_mic", id }),
  removeMic: (id: string) => send(`rm-mic:${id}`, { op: "remove_mic", id }),
  savePreset: (name: string) => send(`save:${name}`, { op: "save_preset", name }),
  loadPreset: (name: string) => send(`load:${name}`, { op: "load_preset", name }),
  deletePreset: (name: string) => send(`del:${name}`, { op: "delete_preset", name }),
  setLabel: (key: string, label: Label) => send(`label:${key}`, { op: "set_label", key, label }),
};
