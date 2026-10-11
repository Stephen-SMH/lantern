import { useCallback, useEffect, useRef, useState } from "react";
import { api, type Action, type Status, type View } from "./api";
import { Board } from "./Board";
import "./App.css";

const MOVES: Record<string, [number, number]> = {
  ArrowUp: [0, -1], ArrowDown: [0, 1], ArrowLeft: [-1, 0], ArrowRight: [1, 0],
  w: [0, -1], s: [0, 1], a: [-1, 0], d: [1, 0],
  k: [0, -1], j: [0, 1], h: [-1, 0], l: [1, 0],
  y: [-1, -1], u: [1, -1], b: [-1, 1], n: [1, 1],
  "8": [0, -1], "2": [0, 1], "4": [-1, 0], "6": [1, 0],
  "7": [-1, -1], "9": [1, -1], "1": [-1, 1], "3": [1, 1],
};

function keyToAction(e: KeyboardEvent): Action | null {
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (e.key === ">" || e.key === "Enter") return { type: "descend" };
  const move = MOVES[key];
  if (move) return { type: "move", dx: move[0], dy: move[1] };
  if (key === "." || key === " " || key === "5") return { type: "wait" };
  if (key === "q") return { type: "drink" };
  if (key === "f" || key === "r") return { type: "refill" };
  return null;
}

function Meter({ label, value, max, tone }: { label: string; value: number; max: number; tone: string }) {
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <div className="meter">
      <div className="meter-head">
        <span>{label}</span>
        <span className="num">
          {value} / {max}
        </span>
      </div>
      <div className="meter-track">
        <div className={`meter-fill ${tone}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function Hud({ view }: { view: View }) {
  const p = view.player;
  const lowOil = p.oil > 0 && p.oil <= 50;
  return (
    <aside className="hud">
      <div className="hud-top">
        <div className="depth">
          <span className="label">Depth</span>
          <span className="depth-num">
            {view.depth}
            <small> / {view.final_depth}</small>
          </span>
        </div>
        <div className="score">
          <span className="label">Score</span>
          <span className="num">{view.score}</span>
        </div>
      </div>

      <Meter label="Health" value={p.hp} max={p.max_hp} tone="hp" />
      <Meter label={p.oil === 0 ? "Oil (out!)" : lowOil ? "Oil (low)" : "Oil"} value={p.oil} max={view.max_oil} tone={lowOil || p.oil === 0 ? "oil low" : "oil"} />
      <Meter label={`Level ${p.level}`} value={p.xp} max={view.next_xp} tone="xp" />

      <dl className="stats">
        <div><dt>Attack</dt><dd>{p.atk}</dd></div>
        <div><dt>Defence</dt><dd>{p.def}</dd></div>
        <div><dt>Light</dt><dd>{view.radius}</dd></div>
        <div><dt>Gold</dt><dd>{p.gold}</dd></div>
      </dl>

      <div className="pack">
        <div className={`slot ${p.potions ? "" : "empty"}`}>
          <span className="slot-glyph potion">!</span>
          <span>Potion ×{p.potions}</span>
          <kbd>Q</kbd>
        </div>
        <div className={`slot ${p.flasks ? "" : "empty"}`}>
          <span className="slot-glyph flask">!</span>
          <span>Oil flask ×{p.flasks}</span>
          <kbd>F</kbd>
        </div>
      </div>

      {view.monsters.length > 0 && (
        <div className="sight">
          <span className="label">In your light</span>
          <ul>
            {view.monsters.map((m, i) => (
              <li key={i}>
                <span className={`glyph m-${m.kind}`}>{m.glyph}</span>
                <span className="cap">{m.name}</span>
                <span className="num dim">
                  {m.hp}/{m.max_hp}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      <ol className="log" aria-live="polite">
        {view.log.map((line, i) => (
          <li key={`${view.turn}-${i}`} style={{ opacity: 0.35 + (0.65 * (i + 1)) / view.log.length }}>
            {line}
          </li>
        ))}
      </ol>

      <p className="hint">
        {view.on_stairs ? (
          <>
            <kbd>&gt;</kbd> descend
          </>
        ) : (
          <>
            Move <kbd>←↑→↓</kbd> <kbd>WASD</kbd> · wait <kbd>.</kbd> · menu <kbd>Esc</kbd>
          </>
        )}
      </p>
    </aside>
  );
}

function Title({ status, onNew, onDaily, onContinue }: { status: Status | null; onNew: () => void; onDaily: () => void; onContinue: () => void }) {
  return (
    <main className="title-screen">
      <div className="title-glow" aria-hidden="true" />
      <div className="title-card">
        <img src="/icon.svg" alt="" className="title-icon" width={88} height={88} />
        <h1>Lantern</h1>
        <p className="tagline">Eight floors down, the Sunstone waits. Your lantern burns a little lower with every step.</p>
        <div className="menu">
          {status?.has_save && (
            <button className="primary" onClick={onContinue} autoFocus>
              Continue run
            </button>
          )}
          <button className={status?.has_save ? "" : "primary"} onClick={onNew} autoFocus={!status?.has_save}>
            New run
          </button>
          <button onClick={onDaily}>
            Daily run <span className="dim">· seed {status?.daily_seed ?? "…"}</span>
          </button>
        </div>
        <ul className="rules">
          <li><b>Light is life.</b> Oil burns each turn. When it runs out, the dark starts to bite.</li>
          <li><b>Wraiths fear the flame.</b> They only strike when your light is weak.</li>
          <li><b>One life.</b> The run saves as you go, and death is permanent.</li>
        </ul>
      </div>
      <section className="scores">
        <h2>Best runs</h2>
        {status && status.scores.length > 0 ? (
          <table>
            <tbody>
              {status.scores.map((s, i) => (
                <tr key={i} className={s.won ? "won" : ""}>
                  <td className="num dim">{i + 1}</td>
                  <td className="num">{s.score}</td>
                  <td>
                    {s.cause}
                    {s.daily && <span className="tag">daily</span>}
                  </td>
                  <td className="num dim">{s.turns} turns</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="dim">No runs yet. The dark is waiting.</p>
        )}
      </section>
    </main>
  );
}

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);

  const refreshStatus = useCallback(() => {
    api.status().then(setStatus).catch((e) => setError(String(e)));
  }, []);
  useEffect(refreshStatus, [refreshStatus]);

  const run = useCallback(async (p: Promise<View>) => {
    if (busy.current) return;
    busy.current = true;
    try {
      setView(await p);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      busy.current = false;
    }
  }, []);

  const toTitle = useCallback(() => {
    setView(null);
    refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (!view) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "Escape") {
        e.preventDefault();
        toTitle();
        return;
      }
      if (view.outcome.state !== "playing") {
        if (e.key === "Enter") {
          e.preventDefault();
          run(api.newGame());
        }
        return;
      }
      const action = keyToAction(e);
      if (action) {
        e.preventDefault();
        run(api.act(action));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, run, toTitle]);

  if (!view) {
    return (
      <>
        <Title
          status={status}
          onNew={() => run(api.newGame())}
          onDaily={() => status && run(api.newGame(status.daily_seed))}
          onContinue={() => run(api.continueGame())}
        />
        {error && <div className="toast">{error}</div>}
      </>
    );
  }

  const over = view.outcome.state !== "playing";
  return (
    <main className="game">
      <section className="stage">
        <Board view={view} />
        {over && (
          <div className={`overlay ${view.outcome.state}`}>
            <div className="overlay-card">
              <h2>{view.outcome.state === "won" ? "The Sunstone is yours" : "Your light goes out"}</h2>
              <p>{view.outcome.state === "dead" ? view.outcome.cause : `You reached the bottom in ${view.turn} turns.`}</p>
              <div className="final">
                <div><span className="label">Score</span><span className="num big">{view.score}</span></div>
                <div><span className="label">Depth</span><span className="num big">{view.depth}</span></div>
                <div><span className="label">Kills</span><span className="num big">{view.player.kills}</span></div>
                <div><span className="label">Gold</span><span className="num big">{view.player.gold}</span></div>
              </div>
              <div className="menu row">
                <button className="primary" onClick={() => run(api.newGame())}>
                  New run <kbd>↵</kbd>
                </button>
                <button onClick={toTitle}>
                  Title <kbd>Esc</kbd>
                </button>
              </div>
            </div>
          </div>
        )}
      </section>
      <Hud view={view} />
      {error && <div className="toast">{error}</div>}
    </main>
  );
}
