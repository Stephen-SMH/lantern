//! The rules of Lantern. Pure and deterministic: the same seed and the same
//! inputs always give the same game. No Tauri and no I/O in here.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const W: i32 = 60;
pub const H: i32 = 34;
pub const FINAL_DEPTH: u32 = 8;
pub const MAX_OIL: i32 = 300;
const START_OIL: i32 = 200;
const FLASK_OIL: i32 = 150;
const POTION_HEAL: i32 = 12;
const LOW_OIL: i32 = 50;
const LOG_LEN: usize = 60;

/// SplitMix64. Small, fast and good enough for dungeon dice.
#[derive(Clone, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `lo..hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        debug_assert!(hi > lo);
        lo + (self.next() % (hi - lo) as u64) as i32
    }

    fn chance(&mut self, percent: i32) -> bool {
        self.range(0, 100) < percent
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Tile {
    Wall,
    Floor,
    Stairs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Rat,
    Bat,
    Goblin,
    Skeleton,
    Wraith,
    Warden,
}

pub struct Stats {
    pub name: &'static str,
    pub glyph: &'static str,
    pub hp: i32,
    atk: i32,
    def: i32,
    xp: i32,
    /// Moves at random half the time.
    erratic: bool,
}

impl Kind {
    pub fn stats(self) -> Stats {
        let s = |name, glyph, hp, atk, def, xp, erratic| Stats { name, glyph, hp, atk, def, xp, erratic };
        match self {
            Kind::Rat => s("rat", "r", 4, 2, 0, 1, false),
            Kind::Bat => s("bat", "b", 3, 2, 0, 1, true),
            Kind::Goblin => s("goblin", "g", 8, 3, 1, 3, false),
            Kind::Skeleton => s("skeleton", "s", 11, 4, 2, 5, false),
            Kind::Wraith => s("wraith", "W", 10, 6, 1, 8, false),
            Kind::Warden => s("Warden", "Ω", 30, 6, 2, 20, false),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Monster {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub awake: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Flask,
    Potion,
    Blade,
    Mail,
    Gold,
    Sunstone,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    pub x: i32,
    pub y: i32,
    pub amount: i32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub oil: i32,
    pub level: i32,
    pub xp: i32,
    pub potions: i32,
    pub flasks: i32,
    pub gold: i32,
    pub kills: i32,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Outcome {
    Playing,
    Dead { cause: String },
    Won,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Move { dx: i32, dy: i32 },
    Wait,
    Descend,
    Drink,
    Refill,
}

#[derive(Clone, Copy)]
struct Room {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Room {
    fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    fn overlaps(&self, o: &Room, gap: i32) -> bool {
        self.x - gap < o.x + o.w && o.x - gap < self.x + self.w && self.y - gap < o.y + o.h && o.y - gap < self.y + self.h
    }
}

fn idx(x: i32, y: i32) -> usize {
    (y * W + x) as usize
}

fn in_bounds(x: i32, y: i32) -> bool {
    (0..W).contains(&x) && (0..H).contains(&y)
}

fn chebyshev(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs().max((ay - by).abs())
}

const DIRS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

#[derive(Clone, Serialize, Deserialize)]
pub struct Game {
    pub seed: u64,
    pub depth: u32,
    pub turn: u32,
    rng: Rng,
    pub tiles: Vec<Tile>,
    pub seen: Vec<bool>,
    /// Recomputed from the map, so it is not saved. Call `update_fov` after loading.
    #[serde(skip)]
    visible: Vec<bool>,
    pub player: Player,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    pub log: Vec<String>,
    pub outcome: Outcome,
}

impl Game {
    pub fn new(seed: u64) -> Game {
        let mut g = Game {
            seed,
            depth: 0,
            turn: 0,
            rng: Rng::new(seed),
            tiles: vec![],
            seen: vec![],
            visible: vec![],
            player: Player {
                x: 0,
                y: 0,
                hp: 20,
                max_hp: 20,
                atk: 3,
                def: 0,
                oil: START_OIL,
                level: 1,
                xp: 0,
                potions: 1,
                flasks: 0,
                gold: 0,
                kills: 0,
            },
            monsters: vec![],
            items: vec![],
            log: vec![],
            outcome: Outcome::Playing,
        };
        g.enter_depth(1);
        g.say("You light your lantern and step into the dark. Find the Sunstone on depth 8.");
        g
    }

    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if in_bounds(x, y) { self.tiles[idx(x, y)] } else { Tile::Wall }
    }

    pub fn is_visible(&self, x: i32, y: i32) -> bool {
        in_bounds(x, y) && self.visible.get(idx(x, y)).copied().unwrap_or(false)
    }

    /// How far the lantern reaches: 8 tiles when full, 1 when it is out.
    pub fn light_radius(&self) -> i32 {
        if self.player.oil <= 0 { 1 } else { 2 + self.player.oil * 6 / MAX_OIL }
    }

    pub fn next_level_xp(&self) -> i32 {
        self.player.level * 8
    }

    pub fn score(&self) -> u32 {
        let p = &self.player;
        let won = if self.outcome == Outcome::Won { 1000 } else { 0 };
        (p.gold + p.kills * 10 + won) as u32 + self.depth * 100
    }

    fn say(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > LOG_LEN {
            self.log.remove(0);
        }
    }

    // ---- generation ----------------------------------------------------

    fn enter_depth(&mut self, depth: u32) {
        self.depth = depth;
        self.monsters.clear();
        self.items.clear();
        let rooms = self.carve();
        let (sx, sy) = rooms[0].center();
        self.player.x = sx;
        self.player.y = sy;

        let last = *rooms.last().unwrap();
        let (ex, ey) = last.center();
        if depth == FINAL_DEPTH {
            self.items.push(Item { kind: ItemKind::Sunstone, x: ex, y: ey, amount: 1 });
            self.monsters.push(Monster { kind: Kind::Warden, x: ex - 1, y: ey, hp: Kind::Warden.stats().hp, awake: false });
        } else {
            self.tiles[idx(ex, ey)] = Tile::Stairs;
        }

        let spawn_rooms = &rooms[1..];
        let monsters = 3 + depth as i32 * 3 / 2;
        for _ in 0..monsters {
            let room = spawn_rooms[self.rng.range(0, spawn_rooms.len() as i32) as usize];
            if let Some((x, y)) = self.free_cell_in(room) {
                let kind = self.roll_monster();
                self.monsters.push(Monster { kind, x, y, hp: kind.stats().hp, awake: false });
            }
        }

        let mut loot = vec![ItemKind::Flask, ItemKind::Flask, ItemKind::Potion];
        if self.rng.chance(50) {
            loot.push(ItemKind::Potion);
        }
        if self.rng.chance(40) {
            loot.push(ItemKind::Blade);
        }
        if self.rng.chance(40) {
            loot.push(ItemKind::Mail);
        }
        for _ in 0..self.rng.range(3, 6) {
            loot.push(ItemKind::Gold);
        }
        for kind in loot {
            let room = rooms[self.rng.range(0, rooms.len() as i32) as usize];
            if let Some((x, y)) = self.free_cell_in(room) {
                let amount = if kind == ItemKind::Gold { self.rng.range(5, 15 + depth as i32 * 5) } else { 1 };
                self.items.push(Item { kind, x, y, amount });
            }
        }

        self.seen = vec![false; (W * H) as usize];
        self.update_fov();
    }

    /// Random rooms joined by L-shaped corridors, plus a couple of loops.
    fn carve(&mut self) -> Vec<Room> {
        self.tiles = vec![Tile::Wall; (W * H) as usize];
        let mut rooms: Vec<Room> = vec![];
        for _ in 0..300 {
            if rooms.len() >= 12 {
                break;
            }
            let w = self.rng.range(5, 13);
            let h = self.rng.range(4, 9);
            let room = Room { x: self.rng.range(1, W - w - 1), y: self.rng.range(1, H - h - 1), w, h };
            if rooms.iter().any(|o| room.overlaps(o, 1)) {
                continue;
            }
            for y in room.y..room.y + room.h {
                for x in room.x..room.x + room.w {
                    self.tiles[idx(x, y)] = Tile::Floor;
                }
            }
            if let Some(prev) = rooms.last() {
                self.tunnel(prev.center(), room.center());
            }
            rooms.push(room);
        }
        for _ in 0..2 {
            let a = rooms[self.rng.range(0, rooms.len() as i32) as usize];
            let b = rooms[self.rng.range(0, rooms.len() as i32) as usize];
            self.tunnel(a.center(), b.center());
        }
        rooms
    }

    fn tunnel(&mut self, (ax, ay): (i32, i32), (bx, by): (i32, i32)) {
        let horizontal_first = self.rng.chance(50);
        let (cx, cy) = if horizontal_first { (bx, ay) } else { (ax, by) };
        for (x0, y0, x1, y1) in [(ax, ay, cx, cy), (cx, cy, bx, by)] {
            for x in x0.min(x1)..=x0.max(x1) {
                for y in y0.min(y1)..=y0.max(y1) {
                    self.tiles[idx(x, y)] = Tile::Floor;
                }
            }
        }
    }

    fn occupied(&self, x: i32, y: i32) -> bool {
        (self.player.x == x && self.player.y == y) || self.monsters.iter().any(|m| m.x == x && m.y == y)
    }

    fn free_cell_in(&mut self, room: Room) -> Option<(i32, i32)> {
        for _ in 0..20 {
            let x = self.rng.range(room.x, room.x + room.w);
            let y = self.rng.range(room.y, room.y + room.h);
            if self.tile(x, y) == Tile::Floor && !self.occupied(x, y) && !self.items.iter().any(|i| i.x == x && i.y == y) {
                return Some((x, y));
            }
        }
        None
    }

    fn roll_monster(&mut self) -> Kind {
        let d = self.depth;
        let table = [
            (Kind::Rat, 1..=3, 30),
            (Kind::Bat, 1..=5, 25),
            (Kind::Goblin, 2..=6, 30),
            (Kind::Skeleton, 4..=8, 25),
            (Kind::Wraith, 5..=8, 20),
        ];
        let options: Vec<(Kind, i32)> = table.into_iter().filter(|(_, r, _)| r.contains(&d)).map(|(k, _, w)| (k, w)).collect();
        let mut roll = self.rng.range(0, options.iter().map(|o| o.1).sum());
        for (kind, weight) in &options {
            if roll < *weight {
                return *kind;
            }
            roll -= weight;
        }
        options[0].0
    }

    // ---- light ---------------------------------------------------------

    /// Ray-cast field of view, limited to the lantern's radius.
    pub fn update_fov(&mut self) {
        let n = (W * H) as usize;
        self.visible = vec![false; n];
        let r = self.light_radius();
        let (px, py) = (self.player.x, self.player.y);
        let lit = |x: i32, y: i32| (x - px).pow(2) + (y - py).pow(2) <= r * r + r;
        self.visible[idx(px, py)] = true;

        let mut targets = vec![];
        for i in -r..=r {
            targets.extend([(px + i, py - r), (px + i, py + r), (px - r, py + i), (px + r, py + i)]);
        }
        for (tx, ty) in targets {
            // Bresenham from the player towards the target.
            let (dx, dy) = ((tx - px).abs(), -(ty - py).abs());
            let (sx, sy) = ((tx - px).signum(), (ty - py).signum());
            let (mut x, mut y, mut err) = (px, py, dx + dy);
            while (x, y) != (tx, ty) {
                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x += sx;
                }
                if e2 <= dx {
                    err += dx;
                    y += sy;
                }
                if !in_bounds(x, y) || !lit(x, y) {
                    break;
                }
                self.visible[idx(x, y)] = true;
                if self.tiles[idx(x, y)] == Tile::Wall {
                    break;
                }
            }
        }

        // Rays can skip the odd wall next to a lit floor; light those too so rooms keep a clean outline.
        let mut walls = vec![];
        for y in 0..H {
            for x in 0..W {
                if self.tiles[idx(x, y)] == Tile::Wall && !self.visible[idx(x, y)] && lit(x, y) {
                    let next_to_lit_floor = DIRS.iter().any(|(dx, dy)| {
                        let (nx, ny) = (x + dx, y + dy);
                        in_bounds(nx, ny) && self.visible[idx(nx, ny)] && self.tiles[idx(nx, ny)] != Tile::Wall
                    });
                    if next_to_lit_floor {
                        walls.push(idx(x, y));
                    }
                }
            }
        }
        for i in walls {
            self.visible[i] = true;
        }
        for i in 0..n {
            if self.visible[i] {
                self.seen[i] = true;
            }
        }
    }

    // ---- turns ---------------------------------------------------------

    /// Applies one player action. Returns false when it did not cost a turn
    /// (bumping a wall, drinking with no potions, and so on).
    pub fn act(&mut self, action: Action) -> bool {
        if self.outcome != Outcome::Playing {
            return false;
        }
        let spent = match action {
            Action::Move { dx, dy } => self.step(dx.clamp(-1, 1), dy.clamp(-1, 1)),
            Action::Wait => true,
            Action::Descend => {
                if self.tile(self.player.x, self.player.y) == Tile::Stairs {
                    self.turn += 1;
                    self.enter_depth(self.depth + 1);
                    let msg = if self.depth == FINAL_DEPTH {
                        "Depth 8. The air is warm here. Something guards the Sunstone.".to_string()
                    } else {
                        format!("You descend to depth {}.", self.depth)
                    };
                    self.say(msg);
                    return true;
                }
                self.say("There are no stairs here.");
                false
            }
            Action::Drink => {
                if self.player.potions == 0 {
                    self.say("You have no potions.");
                    false
                } else {
                    let p = &mut self.player;
                    p.potions -= 1;
                    p.hp = (p.hp + POTION_HEAL).min(p.max_hp);
                    self.say("You drink a potion and feel better.");
                    true
                }
            }
            Action::Refill => {
                if self.player.flasks == 0 {
                    self.say("You have no oil flasks.");
                    false
                } else if self.player.oil >= MAX_OIL {
                    self.say("Your lantern is already full.");
                    false
                } else {
                    let p = &mut self.player;
                    p.flasks -= 1;
                    p.oil = (p.oil + FLASK_OIL).min(MAX_OIL);
                    self.say("You refill the lantern. The light swells.");
                    true
                }
            }
        };
        if spent {
            self.end_turn();
        }
        spent
    }

    fn step(&mut self, dx: i32, dy: i32) -> bool {
        if dx == 0 && dy == 0 {
            return true;
        }
        let (nx, ny) = (self.player.x + dx, self.player.y + dy);
        if let Some(i) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) {
            self.player_attacks(i);
            return true;
        }
        if self.tile(nx, ny) == Tile::Wall {
            return false;
        }
        self.player.x = nx;
        self.player.y = ny;
        self.pick_up();
        if self.tile(nx, ny) == Tile::Stairs {
            self.say("Stairs lead down. Press > to descend.");
        }
        true
    }

    fn pick_up(&mut self) {
        while let Some(i) = self.items.iter().position(|it| it.x == self.player.x && it.y == self.player.y) {
            let item = self.items.remove(i);
            let p = &mut self.player;
            let msg = match item.kind {
                ItemKind::Flask => {
                    p.flasks += 1;
                    "You pick up a flask of oil.".to_string()
                }
                ItemKind::Potion => {
                    p.potions += 1;
                    "You pick up a healing potion.".to_string()
                }
                ItemKind::Blade => {
                    p.atk += 1;
                    "You find a sharper blade. Attack +1.".to_string()
                }
                ItemKind::Mail => {
                    p.def += 1;
                    "You strap on a piece of mail. Defence +1.".to_string()
                }
                ItemKind::Gold => {
                    p.gold += item.amount;
                    format!("You pick up {} gold.", item.amount)
                }
                ItemKind::Sunstone => {
                    self.outcome = Outcome::Won;
                    "The Sunstone blazes in your hand. The dark is beaten. You win!".to_string()
                }
            };
            self.say(msg);
        }
    }

    fn player_attacks(&mut self, i: usize) {
        let s = self.monsters[i].kind.stats();
        self.monsters[i].awake = true;
        if self.rng.chance(10) {
            self.say(format!("You miss the {}.", s.name));
            return;
        }
        let dmg = (self.player.atk + self.rng.range(0, 3) - s.def).max(1);
        self.monsters[i].hp -= dmg;
        if self.monsters[i].hp > 0 {
            self.say(format!("You hit the {} for {dmg}.", s.name));
            return;
        }
        self.monsters.remove(i);
        self.player.kills += 1;
        self.player.xp += s.xp;
        self.say(format!("You slay the {}.", s.name));
        while self.player.xp >= self.next_level_xp() {
            self.player.xp -= self.next_level_xp();
            let p = &mut self.player;
            p.level += 1;
            p.max_hp += 4;
            p.hp = (p.hp + 6).min(p.max_hp);
            p.atk += 1;
            let level = p.level;
            self.say(format!("You reach level {level}. Max HP +4, attack +1."));
        }
    }

    fn hurt_player(&mut self, dmg: i32, cause: impl Into<String>) {
        self.player.hp -= dmg;
        if self.player.hp <= 0 && self.outcome == Outcome::Playing {
            self.player.hp = 0;
            self.outcome = Outcome::Dead { cause: cause.into() };
            self.say("You die. Your lantern rolls away into the dark.");
        }
    }

    fn end_turn(&mut self) {
        self.turn += 1;
        if self.outcome != Outcome::Playing {
            return;
        }
        if self.player.oil > 0 {
            self.player.oil -= 1;
            if self.player.oil == LOW_OIL {
                self.say("Your lantern gutters. Oil is running low.");
            } else if self.player.oil == 0 {
                self.say("Your lantern goes out. Something stirs in the dark.");
            }
        } else if self.turn % 4 == 0 {
            self.say("The dark gnaws at you.");
            let depth = self.depth;
            self.hurt_player(1, format!("Swallowed by the dark on depth {depth}"));
        }
        self.update_fov();
        self.monsters_act();
    }

    /// Steps from every walkable tile to the player, ignoring monsters.
    fn distance_map(&self) -> Vec<i32> {
        let mut dist = vec![i32::MAX; (W * H) as usize];
        let mut queue = VecDeque::new();
        dist[idx(self.player.x, self.player.y)] = 0;
        queue.push_back((self.player.x, self.player.y));
        while let Some((x, y)) = queue.pop_front() {
            let d = dist[idx(x, y)];
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if self.tile(nx, ny) != Tile::Wall && dist[idx(nx, ny)] == i32::MAX {
                    dist[idx(nx, ny)] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        dist
    }

    fn monsters_act(&mut self) {
        let dist = self.distance_map();
        let radius = self.light_radius();
        for i in 0..self.monsters.len() {
            if self.outcome != Outcome::Playing {
                return;
            }
            let m = self.monsters[i].clone();
            let s = m.kind.stats();
            let d = chebyshev(m.x, m.y, self.player.x, self.player.y);
            if !m.awake {
                // Monsters wake when your light falls on them or you get close.
                if self.is_visible(m.x, m.y) || d <= 2 {
                    self.monsters[i].awake = true;
                }
                continue;
            }
            // Wraiths can't stand the light: they keep to its edge until it weakens.
            let flee = m.kind == Kind::Wraith && self.player.oil > 0 && d <= radius - 2;
            if !flee && d == 1 {
                self.monster_attacks(i);
                continue;
            }
            if s.erratic && self.rng.chance(50) {
                let (dx, dy) = DIRS[self.rng.range(0, 8) as usize];
                self.try_move_monster(i, m.x + dx, m.y + dy);
                continue;
            }
            let here = dist[idx(m.x, m.y)];
            let mut best: Option<(i32, i32, i32)> = None;
            for (dx, dy) in DIRS {
                let (nx, ny) = (m.x + dx, m.y + dy);
                if self.tile(nx, ny) == Tile::Wall || self.occupied(nx, ny) {
                    continue;
                }
                let nd = dist[idx(nx, ny)];
                let better = if flee { nd > best.map_or(here, |b| b.2) } else { nd < best.map_or(here, |b| b.2) };
                if better {
                    best = Some((nx, ny, nd));
                }
            }
            if let Some((nx, ny, _)) = best {
                self.monsters[i].x = nx;
                self.monsters[i].y = ny;
            }
        }
    }

    fn try_move_monster(&mut self, i: usize, x: i32, y: i32) {
        if self.tile(x, y) != Tile::Wall && !self.occupied(x, y) {
            self.monsters[i].x = x;
            self.monsters[i].y = y;
        }
    }

    fn monster_attacks(&mut self, i: usize) {
        let s = self.monsters[i].kind.stats();
        if self.rng.chance(15) {
            self.say(format!("The {} misses.", s.name));
            return;
        }
        let dmg = (s.atk + self.rng.range(0, 2) - self.player.def).max(1);
        self.say(format!("The {} hits you for {dmg}.", s.name));
        let depth = self.depth;
        let article = if s.name == "Warden" { "the" } else { "a" };
        self.hurt_player(dmg, format!("Slain by {article} {} on depth {depth}", s.name));
    }

    // ---- view ----------------------------------------------------------

    /// Everything the UI needs to draw one frame. Only what the player can see
    /// or remembers is included, so the UI can't leak the map.
    pub fn view(&self) -> View {
        let mut cells = String::with_capacity((W * H) as usize);
        for i in 0..(W * H) as usize {
            let c = match (self.tiles[i], self.visible[i], self.seen[i]) {
                (_, false, false) => ' ',
                (Tile::Wall, true, _) => 'W',
                (Tile::Floor, true, _) => 'F',
                (Tile::Stairs, true, _) => 'S',
                (Tile::Wall, false, true) => 'w',
                (Tile::Floor, false, true) => 'f',
                (Tile::Stairs, false, true) => 's',
            };
            cells.push(c);
        }
        let monsters = self
            .monsters
            .iter()
            .filter(|m| self.is_visible(m.x, m.y))
            .map(|m| {
                let s = m.kind.stats();
                MonsterView { x: m.x, y: m.y, kind: m.kind, name: s.name, glyph: s.glyph, hp: m.hp, max_hp: s.hp, awake: m.awake }
            })
            .collect();
        let items = self.items.iter().filter(|i| self.is_visible(i.x, i.y)).cloned().collect();
        let start = self.log.len().saturating_sub(8);
        View {
            w: W,
            h: H,
            cells,
            monsters,
            items,
            player: self.player.clone(),
            depth: self.depth,
            final_depth: FINAL_DEPTH,
            turn: self.turn,
            radius: self.light_radius(),
            max_oil: MAX_OIL,
            next_xp: self.next_level_xp(),
            on_stairs: self.tile(self.player.x, self.player.y) == Tile::Stairs,
            log: self.log[start..].to_vec(),
            outcome: self.outcome.clone(),
            score: self.score(),
            seed: self.seed,
        }
    }
}

#[derive(Serialize)]
pub struct MonsterView {
    pub x: i32,
    pub y: i32,
    pub kind: Kind,
    pub name: &'static str,
    pub glyph: &'static str,
    pub hp: i32,
    pub max_hp: i32,
    pub awake: bool,
}

#[derive(Serialize)]
pub struct View {
    pub w: i32,
    pub h: i32,
    pub cells: String,
    pub monsters: Vec<MonsterView>,
    pub items: Vec<Item>,
    pub player: Player,
    pub depth: u32,
    pub final_depth: u32,
    pub turn: u32,
    pub radius: i32,
    pub max_oil: i32,
    pub next_xp: i32,
    pub on_stairs: bool,
    pub log: Vec<String>,
    pub outcome: Outcome,
    pub score: u32,
    pub seed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty walled box with the player in the middle and nothing else.
    fn arena() -> Game {
        let mut g = Game::new(1);
        for y in 0..H {
            for x in 0..W {
                let edge = x == 0 || y == 0 || x == W - 1 || y == H - 1;
                g.tiles[idx(x, y)] = if edge { Tile::Wall } else { Tile::Floor };
            }
        }
        g.monsters.clear();
        g.items.clear();
        g.player.x = W / 2;
        g.player.y = H / 2;
        g.update_fov();
        g
    }

    fn reachable(g: &Game) -> usize {
        g.distance_map().iter().filter(|d| **d != i32::MAX).count()
    }

    #[test]
    fn same_seed_same_dungeon() {
        let (a, b) = (Game::new(42), Game::new(42));
        assert_eq!(a.tiles, b.tiles);
        assert_eq!((a.player.x, a.player.y), (b.player.x, b.player.y));
        assert_ne!(a.tiles, Game::new(43).tiles);
    }

    #[test]
    fn every_floor_is_connected_and_has_an_exit() {
        for seed in 0..200 {
            let g = Game::new(seed);
            let floors = g.tiles.iter().filter(|t| **t != Tile::Wall).count();
            assert_eq!(reachable(&g), floors, "seed {seed} has unreachable floor");
            assert_eq!(g.tiles.iter().filter(|t| **t == Tile::Stairs).count(), 1, "seed {seed}");
            assert!(g.monsters.iter().all(|m| g.tile(m.x, m.y) == Tile::Floor));
        }
    }

    #[test]
    fn walls_block_light() {
        let mut g = arena();
        let (px, py) = (g.player.x, g.player.y);
        for y in 0..H {
            g.tiles[idx(px + 2, y)] = Tile::Wall;
        }
        g.update_fov();
        assert!(g.is_visible(px + 1, py));
        assert!(g.is_visible(px + 2, py), "the wall itself is lit");
        assert!(!g.is_visible(px + 3, py), "but not what is behind it");
        assert!(g.is_visible(px - 3, py));
    }

    #[test]
    fn oil_burns_and_the_light_shrinks() {
        let mut g = arena();
        g.player.oil = MAX_OIL;
        assert_eq!(g.light_radius(), 8);
        g.act(Action::Wait);
        assert_eq!(g.player.oil, MAX_OIL - 1);
        g.player.oil = 1;
        g.act(Action::Wait);
        assert_eq!(g.player.oil, 0);
        assert_eq!(g.light_radius(), 1);
        assert!(!g.is_visible(g.player.x + 2, g.player.y));
    }

    #[test]
    fn the_dark_hurts_without_oil() {
        let mut g = arena();
        g.player.oil = 0;
        let hp = g.player.hp;
        for _ in 0..8 {
            g.act(Action::Wait);
        }
        assert_eq!(g.player.hp, hp - 2);
    }

    #[test]
    fn refill_uses_a_flask() {
        let mut g = arena();
        g.player.oil = 10;
        assert!(!g.act(Action::Refill), "no flask, no turn");
        g.player.flasks = 1;
        assert!(g.act(Action::Refill));
        assert_eq!((g.player.flasks, g.player.oil), (0, 10 + FLASK_OIL - 1));
    }

    #[test]
    fn bumping_attacks_and_kills_give_xp() {
        let mut g = arena();
        let (px, py) = (g.player.x, g.player.y);
        g.monsters.push(Monster { kind: Kind::Rat, x: px + 1, y: py, hp: 1, awake: true });
        g.player.atk = 10;
        while !g.monsters.is_empty() {
            g.act(Action::Move { dx: 1, dy: 0 });
        }
        assert_eq!((g.player.x, g.player.y), (px, py), "attacking does not move you");
        assert_eq!(g.player.kills, 1);
        assert_eq!(g.player.xp, 1);
    }

    #[test]
    fn walls_stop_you_without_costing_a_turn() {
        let mut g = arena();
        g.player.x = 1;
        assert!(!g.act(Action::Move { dx: -1, dy: 0 }));
        assert_eq!((g.player.x, g.turn), (1, 0));
    }

    #[test]
    fn monsters_chase_and_can_kill_you() {
        let mut g = arena();
        let (px, py) = (g.player.x, g.player.y);
        g.monsters.push(Monster { kind: Kind::Goblin, x: px + 5, y: py, hp: 99, awake: true });
        g.player.hp = 3;
        for _ in 0..30 {
            g.act(Action::Wait);
        }
        assert!(matches!(g.outcome, Outcome::Dead { ref cause } if cause.contains("goblin")));
        assert!(!g.act(Action::Wait), "no turns after death");
    }

    #[test]
    fn wraiths_keep_out_of_bright_light() {
        let mut g = arena();
        let (px, py) = (g.player.x, g.player.y);
        g.player.oil = MAX_OIL;
        g.monsters.push(Monster { kind: Kind::Wraith, x: px + 1, y: py, hp: 10, awake: true });
        let hp = g.player.hp;
        for _ in 0..5 {
            g.act(Action::Wait);
        }
        assert_eq!(g.player.hp, hp, "a wraith never strikes in full light");
        let w = &g.monsters[0];
        assert!(chebyshev(w.x, w.y, px, py) > 1);

        g.player.oil = 0;
        g.player.hp = 99;
        for _ in 0..20 {
            g.act(Action::Wait);
        }
        assert!(g.player.hp < 99 - 5, "in the dark it closes in");
    }

    #[test]
    fn stairs_lead_down_and_the_sunstone_wins() {
        let mut g = Game::new(7);
        assert!(!g.act(Action::Descend));
        while g.depth < FINAL_DEPTH {
            let i = g.tiles.iter().position(|t| *t == Tile::Stairs).unwrap();
            g.player.x = i as i32 % W;
            g.player.y = i as i32 / W;
            assert!(g.act(Action::Descend));
        }
        assert!(!g.tiles.contains(&Tile::Stairs), "no stairs on the last floor");
        assert!(g.monsters.iter().any(|m| m.kind == Kind::Warden));
        let stone = g.items.iter().find(|i| i.kind == ItemKind::Sunstone).unwrap().clone();
        g.monsters.clear();
        g.player.x = stone.x - 1;
        g.player.y = stone.y;
        g.act(Action::Move { dx: 1, dy: 0 });
        assert_eq!(g.outcome, Outcome::Won);
        assert!(g.score() >= 1000 + FINAL_DEPTH * 100);
    }

    #[test]
    fn view_hides_what_you_cannot_see() {
        let g = Game::new(3);
        let v = g.view();
        assert_eq!(v.cells.len(), (W * H) as usize);
        assert!(v.cells.contains(' '), "unexplored cells are blank");
        assert!(v.monsters.iter().all(|m| g.is_visible(m.x, m.y)));
    }

    /// A simple bot: fight what is adjacent, grab visible loot, head for the
    /// stairs. Run with `cargo test balance -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn balance_report() {
        let mut deaths = std::collections::BTreeMap::<u32, u32>::new();
        let (mut wins, runs) = (0, 200);
        let mut causes = std::collections::BTreeMap::<String, u32>::new();
        for seed in 0..runs {
            let mut g = Game::new(seed);
            for _ in 0..6000 {
                if g.outcome != Outcome::Playing {
                    break;
                }
                let p = g.player.clone();
                let action = if p.hp < p.max_hp / 2 && p.potions > 0 {
                    Action::Drink
                } else if p.oil < 60 && p.flasks > 0 {
                    Action::Refill
                } else if let Some(m) = g.monsters.iter().find(|m| chebyshev(m.x, m.y, p.x, p.y) == 1) {
                    Action::Move { dx: m.x - p.x, dy: m.y - p.y }
                } else if g.tile(p.x, p.y) == Tile::Stairs {
                    Action::Descend
                } else {
                    let goal = g
                        .items
                        .iter()
                        .find(|i| g.is_visible(i.x, i.y))
                        .map(|i| (i.x, i.y))
                        .or_else(|| g.items.iter().find(|i| i.kind == ItemKind::Sunstone).map(|i| (i.x, i.y)))
                        .unwrap_or_else(|| {
                            let i = g.tiles.iter().position(|t| *t == Tile::Stairs).unwrap();
                            (i as i32 % W, i as i32 / W)
                        });
                    // Walk down a distance map rooted at the goal.
                    let mut tmp = g.clone();
                    tmp.player.x = goal.0;
                    tmp.player.y = goal.1;
                    let dist = tmp.distance_map();
                    let (dx, dy) = DIRS
                        .iter()
                        .copied()
                        .filter(|(dx, dy)| g.tile(p.x + dx, p.y + dy) != Tile::Wall)
                        .min_by_key(|(dx, dy)| dist[idx(p.x + dx, p.y + dy)])
                        .unwrap();
                    Action::Move { dx, dy }
                };
                if !g.act(action) {
                    g.act(Action::Wait);
                }
            }
            match g.outcome {
                Outcome::Won => wins += 1,
                Outcome::Dead { ref cause } => {
                    *deaths.entry(g.depth).or_default() += 1;
                    *causes.entry(cause.split(" on ").next().unwrap().to_string()).or_default() += 1;
                }
                Outcome::Playing => *causes.entry("Still wandering".into()).or_default() += 1,
            }
        }
        println!("bot won {wins}/{runs}; deaths by depth: {deaths:?}");
        println!("causes of death: {causes:?}");
    }

    /// Writes `View` JSON for the README screenshots to `$LANTERN_SHOTS`.
    /// Run with `LANTERN_SHOTS=/tmp/shots cargo test shots -- --ignored`.
    #[test]
    #[ignore]
    fn shots() {
        let Ok(dir) = std::env::var("LANTERN_SHOTS") else { return };
        let write = |name: &str, g: &Game| std::fs::write(format!("{dir}/{name}.json"), serde_json::to_string(&g.view()).unwrap()).unwrap();

        // Mid-run: depth 5, explored, with a fight in the lantern light.
        let mut g = Game::new(2026);
        while g.depth < 5 {
            let i = g.tiles.iter().position(|t| *t == Tile::Stairs).unwrap();
            g.player.x = i as i32 % W;
            g.player.y = i as i32 / W;
            g.act(Action::Descend);
        }
        for (i, t) in g.tiles.iter().enumerate() {
            if *t != Tile::Wall && (i as i32 % W) < W * 2 / 3 {
                g.seen[i] = true;
            }
        }
        g.player = Player { hp: 21, max_hp: 32, atk: 8, def: 2, oil: 230, level: 4, xp: 11, potions: 2, flasks: 1, gold: 214, kills: 23, ..g.player.clone() };
        let (px, py) = (g.player.x, g.player.y);
        g.monsters.retain(|m| chebyshev(m.x, m.y, px, py) > 12);
        for (kind, dx, dy, hp) in [(Kind::Skeleton, 2, -1, 6), (Kind::Goblin, -2, 1, 8), (Kind::Wraith, 4, 1, 10)] {
            if g.tile(px + dx, py + dy) != Tile::Wall {
                g.monsters.push(Monster { kind, x: px + dx, y: py + dy, hp, awake: true });
            }
        }
        if g.tile(px + 1, py + 2) == Tile::Floor {
            g.items.push(Item { kind: ItemKind::Gold, x: px + 1, y: py + 2, amount: 31 });
        }
        g.update_fov();
        g.log = vec![
            "You descend to depth 5.".into(),
            "You pick up a flask of oil.".into(),
            "A goblin snarls at the edge of the light.".into(),
            "You hit the skeleton for 7.".into(),
            "The skeleton hits you for 3.".into(),
        ];
        write("fight", &g);

        // The end: the Sunstone, picked up.
        while g.depth < FINAL_DEPTH {
            let i = g.tiles.iter().position(|t| *t == Tile::Stairs).unwrap();
            g.player.x = i as i32 % W;
            g.player.y = i as i32 / W;
            g.act(Action::Descend);
        }
        let stone = g.items.iter().find(|i| i.kind == ItemKind::Sunstone).unwrap().clone();
        g.monsters.clear();
        g.player.x = stone.x - 1;
        g.player.y = stone.y;
        g.player.kills = 41;
        g.player.gold = 388;
        g.turn = 1734;
        g.act(Action::Move { dx: 1, dy: 0 });
        write("won", &g);
    }

    #[test]
    fn save_round_trip() {
        let mut g = Game::new(9);
        g.act(Action::Move { dx: 1, dy: 0 });
        g.act(Action::Wait);
        let json = serde_json::to_string(&g).unwrap();
        let mut back: Game = serde_json::from_str(&json).unwrap();
        back.update_fov();
        assert_eq!(back.view().cells, g.view().cells);
        // Same RNG state, so both games continue identically.
        for _ in 0..20 {
            g.act(Action::Wait);
            back.act(Action::Wait);
        }
        assert_eq!(serde_json::to_string(&back).unwrap(), serde_json::to_string(&g).unwrap());
    }
}
