import { invoke } from "@tauri-apps/api/core";

export type Kind = "rat" | "bat" | "goblin" | "skeleton" | "wraith" | "warden";
export type ItemKind = "flask" | "potion" | "blade" | "mail" | "gold" | "sunstone";

export interface MonsterView {
  x: number;
  y: number;
  kind: Kind;
  name: string;
  glyph: string;
  hp: number;
  max_hp: number;
  awake: boolean;
}

export interface Item {
  kind: ItemKind;
  x: number;
  y: number;
  amount: number;
}

export interface Player {
  x: number;
  y: number;
  hp: number;
  max_hp: number;
  atk: number;
  def: number;
  oil: number;
  level: number;
  xp: number;
  potions: number;
  flasks: number;
  gold: number;
  kills: number;
}

export type Outcome = { state: "playing" } | { state: "dead"; cause: string } | { state: "won" };

export interface View {
  w: number;
  h: number;
  /** One char per cell. Upper case is lit, lower case is remembered, space is unexplored. W/F/S = wall/floor/stairs. */
  cells: string;
  monsters: MonsterView[];
  items: Item[];
  player: Player;
  depth: number;
  final_depth: number;
  turn: number;
  radius: number;
  max_oil: number;
  next_xp: number;
  on_stairs: boolean;
  log: string[];
  outcome: Outcome;
  score: number;
  seed: number;
}

export interface Score {
  score: number;
  depth: number;
  turns: number;
  won: boolean;
  cause: string;
  seed: number;
  daily: boolean;
  at: number;
}

export interface Status {
  has_save: boolean;
  scores: Score[];
  daily_seed: number;
}

export type Action =
  | { type: "move"; dx: number; dy: number }
  | { type: "wait" }
  | { type: "descend" }
  | { type: "drink" }
  | { type: "refill" };

export const api = {
  status: () => invoke<Status>("status"),
  newGame: (seed?: number) => invoke<View>("new_game", { seed: seed ?? null }),
  continueGame: () => invoke<View>("continue_game"),
  act: (action: Action) => invoke<View>("act", { action }),
};
