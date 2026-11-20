// Mirrors protocol/src (Rust). Update both together when the protocol changes.

export interface Cell {
  volume: number; // gain 0..1
  on: boolean;
}
export interface Row {
  volume: number;
  mute: boolean;
  cells: Record<string, Cell>;
}
export interface Column {
  name: string;
  volume: number;
  mute: boolean;
}
export interface Preset {
  outputs: Record<string, Column>;
  inputs: Record<string, Row>;
  default_route: Record<string, Cell>;
}
export interface Label {
  name?: string | null;
  color?: string | null; // "#rrggbb"
  pinned?: boolean;
}
export interface State {
  sink: string;
  outputs: Record<string, Column>;
  inputs: Record<string, Row>;
  default_route: Record<string, Cell>;
  presets: Record<string, Preset>;
  hotkeys: Record<string, string>;
  labels: Record<string, Label>;
  app_paths: Record<string, string>;
}
export interface Snapshot {
  state: State;
  renders: Record<string, string>;
  captures: Record<string, string>;
  active_inputs: string[];
  active_outputs: string[];
  open_apps: string[];
  playing_apps: string[];
}

export type Request =
  | { op: "snapshot" }
  | { op: "set_input"; key: string; volume?: number; mute?: boolean }
  | { op: "set_output"; id: string; volume?: number; mute?: boolean }
  | { op: "set_cell"; input: string; output: string; volume?: number; on?: boolean }
  | { op: "set_default_cell"; output: string; volume?: number; on?: boolean }
  | { op: "reset_input"; key: string }
  | { op: "add_output"; id: string }
  | { op: "remove_output"; id: string }
  | { op: "add_mic"; id: string }
  | { op: "remove_mic"; id: string }
  | { op: "toggle_mics" }
  | { op: "save_preset"; name: string }
  | { op: "load_preset"; name: string }
  | { op: "delete_preset"; name: string }
  | { op: "set_hotkey"; combo: string; action: string | null }
  | { op: "set_label"; key: string; label: Label };

export type Response = ({ type: "snapshot" } & Snapshot) | { type: "ok" } | { type: "error"; message: string };
