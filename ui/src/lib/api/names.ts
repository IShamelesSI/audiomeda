// Display names and colors. App keys are "app:<exe>", mic keys "mic:<device id>",
// outputs are render device ids. A user's custom label always wins.

import type { Snapshot } from "./types";

const KNOWN: Record<string, string> = {
  msedge: "Edge",
  chrome: "Chrome",
  firefox: "Firefox",
  opera: "Opera",
  discord: "Discord",
  spotify: "Spotify",
  steam: "Steam",
  vlc: "VLC",
  obs64: "OBS",
};

/** "Speakers (HyperX Quadcast)" -> "HyperX Quadcast"; drops generic driver names in brackets. */
export function shortDevice(name: string): string {
  const inner = name.match(/\(([^)]+)\)\s*$/)?.[1];
  if (!inner || /high definition audio|virtual audio|vb-audio|voicemeeter/i.test(inner)) {
    return name.replace(/\s*\([^)]*\)\s*$/, "");
  }
  return inner;
}

/** Virtual cables and similar software devices (by their Windows name). */
export function isVirtual(deviceName: string): boolean {
  return /cable|vb-audio|virtual|voicemeeter|voicemod|steam streaming/i.test(deviceName);
}

function original(key: string, snap: Snapshot): string {
  if (key.startsWith("app:")) {
    const exe = key.slice(4).replace(/\.exe$/, "");
    return KNOWN[exe] ?? exe.charAt(0).toUpperCase() + exe.slice(1);
  }
  if (key.startsWith("mic:")) return shortDevice(snap.captures[key.slice(4)] ?? "Microphone");
  return shortDevice(snap.state.outputs[key]?.name ?? snap.renders[key] ?? key);
}

/** The name to show: custom label, else the device/app name. */
export function displayName(key: string, snap: Snapshot): string {
  return snap.state.labels[key]?.name || original(key, snap);
}

/** "Custom (Original)" when renamed, else just the original. */
export function fullName(key: string, snap: Snapshot): string {
  const custom = snap.state.labels[key]?.name;
  return custom ? `${custom} (${original(key, snap)})` : original(key, snap);
}

export function originalName(key: string, snap: Snapshot): string {
  return original(key, snap);
}

export function colorOf(key: string, snap: Snapshot): string | undefined {
  return snap.state.labels[key]?.color ?? undefined;
}
