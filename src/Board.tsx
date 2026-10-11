import { useEffect, useRef } from "react";
import type { ItemKind, Kind, View } from "./api";

const MONSTER_COLOR: Record<Kind, string> = {
  rat: "#c4a484",
  bat: "#b197fc",
  goblin: "#8ce99a",
  skeleton: "#f1f3f5",
  wraith: "#66d9e8",
  warden: "#ff6b6b",
};

const ITEM_GLYPH: Record<ItemKind, [string, string]> = {
  flask: ["!", "#ffc94d"],
  potion: ["!", "#ff6b6b"],
  blade: ["/", "#dee2e6"],
  mail: ["[", "#a5b4c8"],
  gold: ["$", "#ffd43b"],
  sunstone: ["*", "#fff3bf"],
};

const FONT = '"SF Mono", Menlo, Consolas, "DejaVu Sans Mono", monospace';

function mix(a: [number, number, number], b: [number, number, number], t: number) {
  const c = a.map((v, i) => Math.round(v + (b[i] - v) * t));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

/** Smooth noise for the flame: a few slow sines, so it wavers instead of strobing. */
function flicker(t: number) {
  return Math.sin(t * 0.0031) * 0.18 + Math.sin(t * 0.0077 + 1.3) * 0.12 + Math.sin(t * 0.0173 + 0.4) * 0.06;
}

export function Board({ view }: { view: View }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewRef = useRef(view);
  viewRef.current = view;

  useEffect(() => {
    const canvas = canvasRef.current!;
    const ctx = canvas.getContext("2d")!;
    let frame = 0;
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const draw = (t: number) => {
      const v = viewRef.current;
      const parent = canvas.parentElement!;
      const dpr = window.devicePixelRatio || 1;
      const cell = Math.max(8, Math.floor(Math.min(parent.clientWidth / v.w, parent.clientHeight / v.h)));
      const cw = cell * v.w;
      const ch = cell * v.h;
      if (canvas.width !== cw * dpr || canvas.height !== ch * dpr) {
        canvas.width = cw * dpr;
        canvas.height = ch * dpr;
        canvas.style.width = `${cw}px`;
        canvas.style.height = `${ch}px`;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.fillStyle = "#07080b";
      ctx.fillRect(0, 0, cw, ch);

      const { x: px, y: py } = v.player;
      const dead = v.outcome.state === "dead";
      const reach = dead ? 0.6 : v.radius + 0.6 + (reduceMotion ? 0 : flicker(t));
      const lightAt = (x: number, y: number) => {
        const d = Math.hypot(x - px, y - py) / reach;
        return Math.max(0.12, 1 - d * d);
      };

      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      const glyph = (ch: string, x: number, y: number, color: string, scale = 0.78, weight = 600) => {
        ctx.font = `${weight} ${Math.round(cell * scale)}px ${FONT}`;
        ctx.fillStyle = color;
        ctx.fillText(ch, x * cell + cell / 2, y * cell + cell / 2 + 1);
      };

      for (let y = 0; y < v.h; y++) {
        for (let x = 0; x < v.w; x++) {
          const c = v.cells[y * v.w + x];
          if (c === " ") continue;
          const sx = x * cell;
          const sy = y * cell;
          if (c === "w") {
            ctx.fillStyle = "#151a24";
            ctx.fillRect(sx, sy, cell, cell);
          } else if (c === "f" || c === "s") {
            ctx.fillStyle = "#0c0f15";
            ctx.fillRect(sx, sy, cell, cell);
            if (c === "s") glyph(">", x, y, "#3d4a63");
            else {
              ctx.fillStyle = "#1c2230";
              ctx.fillRect(sx + cell / 2 - 1, sy + cell / 2 - 1, 2, 2);
            }
          } else {
            const l = lightAt(x, y);
            if (c === "W") {
              ctx.fillStyle = mix([26, 22, 18], [118, 86, 50], l);
              ctx.fillRect(sx, sy, cell, cell);
              ctx.fillStyle = mix([34, 28, 22], [156, 118, 70], l);
              ctx.fillRect(sx, sy, cell, Math.max(2, cell * 0.18));
            } else {
              ctx.fillStyle = mix([14, 12, 10], [64, 44, 22], l);
              ctx.fillRect(sx, sy, cell, cell);
              if (c === "S") glyph(">", x, y, mix([90, 80, 60], [255, 236, 179], l), 0.85, 800);
              else {
                ctx.fillStyle = `rgba(255, 200, 120, ${0.12 + l * 0.35})`;
                ctx.fillRect(sx + cell / 2 - 1, sy + cell / 2 - 1, 2, 2);
              }
            }
          }
        }
      }

      for (const it of v.items) {
        const [g, color] = ITEM_GLYPH[it.kind];
        if (it.kind === "sunstone") {
          const cx = it.x * cell + cell / 2;
          const cy = it.y * cell + cell / 2;
          const halo = ctx.createRadialGradient(cx, cy, 0, cx, cy, cell * 2.5);
          halo.addColorStop(0, "rgba(255, 236, 153, 0.55)");
          halo.addColorStop(1, "rgba(255, 236, 153, 0)");
          ctx.fillStyle = halo;
          ctx.fillRect(cx - cell * 3, cy - cell * 3, cell * 6, cell * 6);
        }
        glyph(g, it.x, it.y, color, 0.85, 800);
      }

      for (const m of v.monsters) {
        glyph(m.glyph, m.x, m.y, MONSTER_COLOR[m.kind], m.kind === "warden" ? 0.95 : 0.85, 800);
        if (m.hp < m.max_hp) {
          const w = cell - 4;
          ctx.fillStyle = "rgba(0,0,0,0.6)";
          ctx.fillRect(m.x * cell + 2, m.y * cell + cell - 3, w, 2);
          ctx.fillStyle = "#ff6b6b";
          ctx.fillRect(m.x * cell + 2, m.y * cell + cell - 3, (w * Math.max(0, m.hp)) / m.max_hp, 2);
        }
      }

      // The warm glow of the lantern, added on top of everything it touches.
      const cx = px * cell + cell / 2;
      const cy = py * cell + cell / 2;
      const glow = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.max(cell, reach * cell));
      glow.addColorStop(0, `rgba(255, 190, 90, ${dead ? 0.05 : 0.22})`);
      glow.addColorStop(0.6, "rgba(255, 150, 40, 0.06)");
      glow.addColorStop(1, "rgba(255, 150, 40, 0)");
      ctx.globalCompositeOperation = "lighter";
      ctx.fillStyle = glow;
      ctx.fillRect(cx - reach * cell, cy - reach * cell, reach * cell * 2, reach * cell * 2);
      ctx.globalCompositeOperation = "source-over";

      glyph("@", px, py, dead ? "#6b6b6b" : "#fff8e1", 0.9, 800);

      frame = requestAnimationFrame(draw);
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, []);

  return <canvas ref={canvasRef} className="board" aria-label="Dungeon map" />;
}
