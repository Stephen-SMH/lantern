mod game;

use game::{Action, Game, Outcome, View};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

const MAX_SCORES: usize = 10;

#[derive(Default)]
struct Session(Mutex<Option<Game>>);

#[derive(Clone, Serialize, Deserialize)]
struct Score {
    score: u32,
    depth: u32,
    turns: u32,
    won: bool,
    cause: String,
    seed: u64,
    daily: bool,
    at: u64,
}

#[derive(Serialize)]
struct Status {
    has_save: bool,
    scores: Vec<Score>,
    daily_seed: u32,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Everyone gets the same dungeon on the same UTC day. Kept under 2^32 so it
/// survives the trip through a JavaScript number.
fn daily_seed() -> u32 {
    let day = now_secs() / 86_400;
    (day.wrapping_mul(0x9E37_79B9) ^ 0x4C41_4E54) as u32
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn save_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("save.json"))
}

fn scores_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("scores.json"))
}

fn write_save(app: &AppHandle, game: &Game) -> Result<(), String> {
    let json = serde_json::to_vec(game).map_err(|e| e.to_string())?;
    fs::write(save_path(app)?, json).map_err(|e| e.to_string())
}

fn read_scores(app: &AppHandle) -> Vec<Score> {
    scores_path(app)
        .ok()
        .and_then(|p| fs::read(p).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Permadeath: a finished run goes on the board and its save is deleted.
fn finish_run(app: &AppHandle, game: &Game) -> Result<(), String> {
    let mut scores = read_scores(app);
    scores.push(Score {
        score: game.score(),
        depth: game.depth,
        turns: game.turn,
        won: game.outcome == Outcome::Won,
        cause: match &game.outcome {
            Outcome::Dead { cause } => cause.clone(),
            _ => "Carried the Sunstone out of the dark".into(),
        },
        seed: game.seed,
        daily: game.seed == daily_seed() as u64,
        at: now_secs(),
    });
    scores.sort_by(|a, b| b.score.cmp(&a.score));
    scores.truncate(MAX_SCORES);
    let json = serde_json::to_vec_pretty(&scores).map_err(|e| e.to_string())?;
    fs::write(scores_path(app)?, json).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(save_path(app)?);
    Ok(())
}

#[tauri::command]
fn status(app: AppHandle) -> Result<Status, String> {
    Ok(Status { has_save: save_path(&app)?.exists(), scores: read_scores(&app), daily_seed: daily_seed() })
}

#[tauri::command]
fn new_game(app: AppHandle, session: State<Session>, seed: Option<u32>) -> Result<View, String> {
    let seed = seed.unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos() ^ d.as_secs() as u32).unwrap_or(1));
    let game = Game::new(seed as u64);
    write_save(&app, &game)?;
    let view = game.view();
    *session.0.lock().unwrap() = Some(game);
    Ok(view)
}

#[tauri::command]
fn continue_game(app: AppHandle, session: State<Session>) -> Result<View, String> {
    let mut guard = session.0.lock().unwrap();
    if guard.is_none() {
        let bytes = fs::read(save_path(&app)?).map_err(|_| "No saved run to continue.".to_string())?;
        let mut game: Game = serde_json::from_slice(&bytes).map_err(|e| format!("The save file is damaged: {e}"))?;
        game.update_fov();
        *guard = Some(game);
    }
    Ok(guard.as_ref().unwrap().view())
}

#[tauri::command]
fn act(app: AppHandle, session: State<Session>, action: Action) -> Result<View, String> {
    let mut guard = session.0.lock().unwrap();
    let game = guard.as_mut().ok_or("No run in progress.")?;
    if game.act(action) {
        if game.outcome == Outcome::Playing {
            write_save(&app, game)?;
        } else {
            finish_run(&app, game)?;
        }
    }
    Ok(game.view())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Session::default())
        .invoke_handler(tauri::generate_handler![status, new_game, continue_game, act])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
