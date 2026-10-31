"use client";

import { useEffect, useRef, type ReactNode } from "react";
import { cn } from "@/lib/utils";

type LinkGroup = { title: string; links: { label: string; href: string }[] };

interface HalftoneWordmarkFooterProps {
  word?: string;
  studioLine?: ReactNode;
  groups?: LinkGroup[];
  bottomLine?: ReactNode;
  className?: string;
}

/* Dot colours: resting dots are cool, displaced dots heat up toward pink and orange. */
const REST: [number, number, number] = [124, 98, 196]; // royal violet
const REST_LIGHT: [number, number, number] = [96, 70, 170]; // deeper violet for light mode
const MID: [number, number, number] = [203, 120, 230]; // orchid
const HOT: [number, number, number] = [242, 193, 78]; // gold

const mix = (a: number[], b: number[], t: number) => a.map((v, i) => Math.round(v + (b[i] - v) * t));

function HalftoneWord({ word }: { word: string }) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const wrap = wrapRef.current;
    const canvas = canvasRef.current;
    if (!wrap || !canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    let W = 0, H = 0, dpr = 1, gap = 9;
    let hx = new Float32Array(0), hy = new Float32Array(0), px = new Float32Array(0), py = new Float32Array(0);
    let vx = new Float32Array(0), vy = new Float32Array(0), rad = new Float32Array(0);
    let count = 0;
    const pointer = { x: -9999, y: -9999, active: false };
    const shocks: { x: number; y: number; t: number }[] = [];
    let raf = 0;
    let fontFamily = "Inter Tight, Inter, system-ui, sans-serif";

    const build = () => {
      const rect = wrap.getBoundingClientRect();
      W = Math.max(1, Math.floor(rect.width));
      // Fit the word to the full width, then size the band so the letters fill it
      // and the bottom edge crops them slightly.
      const probe = document.createElement("canvas").getContext("2d")!;
      probe.font = `800 100px ${fontFamily}`;
      const size = (100 * (W * 0.98)) / probe.measureText(word).width;
      H = Math.max(60, Math.round(size * 0.64));
      wrap.style.height = H + "px";
      dpr = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = W * dpr;
      canvas.height = H * dpr;
      canvas.style.width = W + "px";
      canvas.style.height = H + "px";
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      gap = W < 640 ? 5 : W < 1100 ? 7 : 8;

      const off = document.createElement("canvas");
      off.width = W;
      off.height = H;
      const o = off.getContext("2d", { willReadFrequently: true })!;
      o.font = `800 ${size}px ${fontFamily}`;
      o.fillStyle = "#fff";
      o.textBaseline = "alphabetic";
      o.textAlign = "center";
      o.fillText(word, W / 2, size * 0.77);
      const data = o.getImageData(0, 0, W, H).data;

      const xs: number[] = [], ys: number[] = [], rs: number[] = [];
      for (let y = gap / 2; y < H; y += gap) {
        for (let x = gap / 2; x < W; x += gap) {
          const a = data[(Math.floor(y) * W + Math.floor(x)) * 4 + 3] / 255;
          if (a > 0.05) {
            xs.push(x);
            ys.push(y);
            // halftone: dots shrink toward the top of the word
            rs.push((gap / 2) * (0.45 + 0.5 * a * (0.55 + 0.45 * (y / H))));
          }
        }
      }
      count = xs.length;
      hx = Float32Array.from(xs);
      hy = Float32Array.from(ys);
      px = Float32Array.from(xs);
      py = Float32Array.from(ys);
      vx = new Float32Array(count);
      vy = new Float32Array(count);
      rad = Float32Array.from(rs);
    };

    const draw = () => {
      ctx.clearRect(0, 0, W, H);
      const light = document.documentElement.classList.contains("light");
      const rest = light ? REST_LIGHT : REST;
      for (let i = 0; i < count; i++) {
        const dx = px[i] - hx[i], dy = py[i] - hy[i];
        const disp = Math.min(1, Math.sqrt(dx * dx + dy * dy) / 40);
        const c = disp < 0.5 ? mix(rest, MID, disp * 2) : mix(MID, HOT, (disp - 0.5) * 2);
        ctx.fillStyle = `rgba(${c[0]},${c[1]},${c[2]},${0.55 + 0.45 * disp})`;
        ctx.beginPath();
        ctx.arc(px[i], py[i], rad[i] * (1 + disp * 0.35), 0, Math.PI * 2);
        ctx.fill();
      }
    };

    const step = () => {
      raf = 0;
      const R = Math.max(90, W * 0.08);
      const now = performance.now();
      let moving = false;
      for (let s = shocks.length - 1; s >= 0; s--) if (now - shocks[s].t > 1400) shocks.splice(s, 1);
      for (let i = 0; i < count; i++) {
        let fx = (hx[i] - px[i]) * 0.06;
        let fy = (hy[i] - py[i]) * 0.06;
        if (pointer.active) {
          const dx = px[i] - pointer.x, dy = py[i] - pointer.y;
          const d2 = dx * dx + dy * dy;
          if (d2 < R * R) {
            const d = Math.sqrt(d2) || 1;
            const f = (1 - d / R) * 3.2;
            fx += (dx / d) * f;
            fy += (dy / d) * f;
          }
        }
        for (const sh of shocks) {
          const ringR = ((now - sh.t) / 1000) * 900;
          const dx = px[i] - sh.x, dy = py[i] - sh.y;
          const d = Math.sqrt(dx * dx + dy * dy) || 1;
          const band = Math.abs(d - ringR);
          if (band < 40) {
            const f = (1 - band / 40) * 4 * (1 - (now - sh.t) / 1400);
            fx += (dx / d) * f;
            fy += (dy / d) * f;
          }
        }
        vx[i] = (vx[i] + fx) * 0.82;
        vy[i] = (vy[i] + fy) * 0.82;
        px[i] += vx[i];
        py[i] += vy[i];
        if (!moving && (Math.abs(vx[i]) > 0.02 || Math.abs(vy[i]) > 0.02 || Math.abs(px[i] - hx[i]) > 0.3)) moving = true;
      }
      draw();
      if (moving || pointer.active || shocks.length) raf = requestAnimationFrame(step);
    };
    const kick = () => {
      if (!raf && !reduce) raf = requestAnimationFrame(step);
    };

    const local = (e: PointerEvent) => {
      const r = canvas.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };
    const onMove = (e: PointerEvent) => {
      const p = local(e);
      pointer.x = p.x;
      pointer.y = p.y;
      pointer.active = true;
      kick();
    };
    const onLeave = () => {
      pointer.active = false;
      kick();
    };
    const onDown = (e: PointerEvent) => {
      const p = local(e);
      shocks.push({ x: p.x, y: p.y, t: performance.now() });
      kick();
    };

    const init = () => {
      build();
      draw();
    };
    init();
    // re-render once the display face has loaded, so the dots trace the real letterforms
    document.fonts?.ready.then(() => {
      if (document.fonts.check(`800 40px "Inter Tight"`)) fontFamily = '"Inter Tight", Inter, system-ui, sans-serif';
      init();
    });
    let lastW = 0;
    // redraw when the theme flips
    const mo = new MutationObserver(() => draw());
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    const ro = new ResizeObserver(([entry]) => {
      const w = Math.floor(entry.contentRect.width);
      if (w !== lastW) {
        lastW = w;
        init();
      }
    });
    ro.observe(wrap);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerleave", onLeave);
    canvas.addEventListener("pointerdown", onDown);
    return () => {
      ro.disconnect();
      mo.disconnect();
      cancelAnimationFrame(raf);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerleave", onLeave);
      canvas.removeEventListener("pointerdown", onDown);
    };
  }, [word]);

  return (
    <div ref={wrapRef} className="relative h-[clamp(60px,14vw,260px)] w-full">
      <canvas ref={canvasRef} role="img" aria-label={word} className="absolute inset-0 block touch-none" />
    </div>
  );
}

/* Fine print-grain overlay, as an inline SVG noise texture. */
const GRAIN =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/></filter><rect width='100%' height='100%' filter='url(%23n)' opacity='0.5'/></svg>\")";

export default function HalftoneWordmarkFooter({
  word = "AGENTMINDER",
  studioLine,
  groups = [],
  bottomLine,
  className,
}: HalftoneWordmarkFooterProps) {
  return (
    <footer className={cn("relative overflow-hidden bg-neutral-950 text-neutral-300", className)}>
      <div aria-hidden="true" className="pointer-events-none absolute inset-0 opacity-[0.07] mix-blend-overlay" style={{ backgroundImage: GRAIN }} />
      <div className="relative mx-auto flex max-w-6xl flex-col gap-12 px-5 pb-10 pt-20">
        <div className="flex flex-col gap-12 lg:flex-row lg:justify-between">
          <div className="max-w-sm text-sm leading-relaxed text-neutral-400">{studioLine}</div>
          <nav aria-label="Footer" className="grid grid-cols-2 gap-x-14 gap-y-10 sm:grid-cols-3">
            {groups.map((g) => (
              <div key={g.title} className="flex flex-col gap-3">
                <p className="font-mono text-[11px] uppercase tracking-[0.28em] text-neutral-500">{g.title}</p>
                <ul className="flex flex-col gap-2 text-sm">
                  {g.links.map((l) => (
                    <li key={l.label}>
                      <a href={l.href} className="text-neutral-300 transition-colors hover:text-white">
                        {l.label}
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </nav>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-4 border-t border-white/10 pt-6 text-xs text-neutral-500">
          <span>{bottomLine}</span>
          <a href="#top" className="inline-flex min-h-11 items-center gap-2 rounded-full border border-white/10 px-4 text-neutral-300 transition-colors hover:border-white/30 hover:text-white">
            Back to top
            <svg viewBox="0 0 16 16" className="h-3.5 w-3.5" aria-hidden="true">
              <path d="M8 13V3M3.5 7.5L8 3l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </a>
        </div>
      </div>
      <HalftoneWord word={word} />
    </footer>
  );
}
