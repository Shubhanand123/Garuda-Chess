/**
 * Scroll-driven 3D hero: one large black lacquer queen alone in the centre.
 * Scrolling moves the camera along a spiral around her (rising, circling, closing in);
 * time adds a slow turn and float. `progress` (0..1) comes from the page.
 */
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";

type Opts = { reduced?: boolean; light?: boolean };

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const clamp01 = (x: number) => Math.min(1, Math.max(0, x));
const ease = (t: number) => t * t * (3 - 2 * t);

function lathe(profile: [number, number][], seg = 160) {
  return new THREE.LatheGeometry(profile.map(([x, y]) => new THREE.Vector2(x, y)), seg);
}

// Staunton queen profile (radius, height)
const BODY: [number, number][] = [
  [0, 0], [1.16, 0], [1.19, 0.05], [1.14, 0.11], [1.16, 0.19], [1.05, 0.25], [0.99, 0.3], [1.05, 0.36], [1.0, 0.42],
  [0.84, 0.48], [0.7, 0.54], [0.64, 0.6], [0.66, 0.65], [0.58, 0.72], [0.52, 0.92], [0.45, 1.25], [0.4, 1.6],
  [0.37, 1.92], [0.38, 2.0], [0.52, 2.04], [0.64, 2.09], [0.66, 2.13], [0.6, 2.17], [0.44, 2.2], [0.5, 2.25],
  [0.47, 2.3], [0.41, 2.36], [0.44, 2.52], [0.52, 2.7], [0.63, 2.86], [0.73, 2.99], [0.76, 3.03], [0.72, 3.06],
  [0.6, 3.04], [0.42, 3.0], [0, 2.99],
];
const DOME: [number, number][] = [
  [0, 2.98], [0.4, 2.98], [0.38, 3.08], [0.32, 3.18], [0.22, 3.26], [0.12, 3.3], [0.1, 3.34], [0.16, 3.37],
  [0.1, 3.41], [0.07, 3.44], [0, 3.44],
];

function shadowTexture() {
  const c = document.createElement("canvas");
  c.width = c.height = 256;
  const g = c.getContext("2d")!;
  const r = g.createRadialGradient(128, 128, 0, 128, 128, 128);
  r.addColorStop(0, "rgba(0,0,0,0.55)");
  r.addColorStop(0.45, "rgba(0,0,0,0.25)");
  r.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = r;
  g.fillRect(0, 0, 256, 256);
  return new THREE.CanvasTexture(c);
}

function glowTexture() {
  const c = document.createElement("canvas");
  c.width = c.height = 256;
  const g = c.getContext("2d")!;
  const r = g.createRadialGradient(128, 128, 0, 128, 128, 128);
  r.addColorStop(0, "rgba(255,255,255,0.22)");
  r.addColorStop(0.4, "rgba(200,205,220,0.07)");
  r.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = r;
  g.fillRect(0, 0, 256, 256);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

const DPR0 = window.devicePixelRatio || 1;
const MOBILE = matchMedia("(pointer: coarse)").matches || Math.min(screen.width, screen.height) < 700;

export function mountQueenHero(canvas: HTMLCanvasElement, getProgress: () => number, opts: Opts = {}) {
  let renderer: THREE.WebGLRenderer;
  try {
    // MSAA is wasted on high-DPR screens; skip it there
    renderer = new THREE.WebGLRenderer({ canvas, antialias: DPR0 < 1.5, alpha: true, powerPreference: "high-performance" });
  } catch {
    return null;
  }
  let dpr = Math.min(DPR0, MOBILE ? 1.25 : 1.5);
  renderer.setPixelRatio(dpr);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.1;
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.02).texture;
  scene.environmentIntensity = 0.45;

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);

  // studio lights: soft key, two strong cool-white rims to carve the black silhouette
  scene.add(new THREE.HemisphereLight(0xffffff, 0x101014, 0.35));
  const key = new THREE.DirectionalLight(0xffffff, 1.1);
  key.position.set(4, 8, 6);
  scene.add(key);
  const rimL = new THREE.DirectionalLight(0xdfe8ff, 4.2);
  rimL.position.set(-5, 5, -5);
  scene.add(rimL);
  const rimR = new THREE.DirectionalLight(0xffffff, 3.4);
  rimR.position.set(5, 3.5, -4.5);
  scene.add(rimR);

  const lacquer = new THREE.MeshPhysicalMaterial({ color: 0x060608, metalness: 0.05, roughness: 0.2, clearcoat: 1, clearcoatRoughness: 0.04, reflectivity: 0.6 });
  const satin = new THREE.MeshStandardMaterial({ color: 0x111116, metalness: 0.2, roughness: 0.38 });
  const small = new THREE.MeshStandardMaterial({ color: 0x08080a, metalness: 0.15, roughness: 0.14 });

  const queen = new THREE.Group();
  queen.add(new THREE.Mesh(lathe(BODY, 96), lacquer));
  queen.add(new THREE.Mesh(lathe(DOME, 48), satin));

  // coronet: ten points around the rim, each with a small ball
  const spike = new THREE.ConeGeometry(0.075, 0.26, 14);
  const ball = new THREE.SphereGeometry(0.068, 14, 10);
  const SP = 10;
  for (let i = 0; i < SP; i++) {
    const a = (i / SP) * Math.PI * 2;
    const x = Math.cos(a) * 0.7, z = Math.sin(a) * 0.7;
    const s = new THREE.Mesh(spike, small);
    s.position.set(x, 3.15, z);
    s.lookAt(x * 1.25, 3.6, z * 1.25);
    s.rotateX(Math.PI / 2);
    queen.add(s);
    const p = new THREE.Mesh(ball, small);
    p.position.set(x * 1.07, 3.29, z * 1.07);
    queen.add(p);
  }
  // finial orb
  const orb = new THREE.Mesh(new THREE.SphereGeometry(0.13, 24, 16), small);
  orb.position.y = 3.56;
  queen.add(orb);
  const tip = new THREE.Mesh(new THREE.SphereGeometry(0.05, 12, 8), small);
  tip.position.y = 3.72;
  queen.add(tip);
  scene.add(queen);

  // soft contact shadow + warm glow behind
  const shadow = new THREE.Mesh(new THREE.PlaneGeometry(4.2, 4.2), new THREE.MeshBasicMaterial({ map: shadowTexture(), transparent: true, depthWrite: false }));
  shadow.rotation.x = -Math.PI / 2;
  shadow.position.y = 0.002;
  scene.add(shadow);
  const glow = new THREE.Sprite(new THREE.SpriteMaterial({ map: glowTexture(), transparent: true, depthWrite: false, blending: opts.light ? THREE.NormalBlending : THREE.AdditiveBlending, opacity: opts.light ? 0.0 : 1 }));
  glow.scale.set(9, 9, 1);
  glow.position.set(0, 2, -2.5);
  scene.add(glow);

  let portrait = false;
  function frame(time: number) {
    const p = opts.reduced ? 0.15 : clamp01(getProgress());
    const t = opts.reduced ? 0 : time / 1000;
    const e = ease(p);

    queen.rotation.y = t * 0.18 + p * Math.PI * 0.6;
    const lift = Math.sin(t * 1.1) * 0.04;
    queen.position.y = lift;
    shadow.scale.setScalar(1 - lift * 1.5);

    // camera spirals once and a quarter around her, rising then settling close at crown height
    const a = -Math.PI / 2 + 0.35 + p * Math.PI * 2.5 + t * 0.02;
    const rad = lerp(11.5, 8.0, e) * (portrait ? 1.35 : 1);
    const h = lerp(1.3, 5.2, Math.sin(p * Math.PI)) + lerp(1.4, 1.8, e);
    camera.position.set(Math.cos(a) * rad, h, Math.sin(a) * rad);
    const look = lerp(1.75, 2.0, e) - (portrait ? 0.9 : 0);
    camera.lookAt(0, look, 0);
    glow.position.set(-Math.cos(a) * 2.5, 2.1, -Math.sin(a) * 2.5);

    renderer.render(scene, camera);
  }

  function resize() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    if (!w || !h) return;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    portrait = w < h;
    camera.fov = portrait ? 36 : 30;
    camera.updateProjectionMatrix();
  }
  const ro = new ResizeObserver(resize);
  ro.observe(canvas);
  resize();

  let visible = true, raf = 0, lastP = -1, lastDraw = 0, slow = 0, frames = 0;
  const io = new IntersectionObserver((e) => { visible = e[0].isIntersecting; if (visible && !raf) loop(0); }, { rootMargin: "100px" });
  io.observe(canvas);
  function loop(time: number) {
    raf = 0;
    if (!visible || document.hidden) return;
    const now = time || performance.now();
    const p = getProgress();
    const moving = Math.abs(p - lastP) > 1e-4;
    // idle drift only needs ~30 fps; scrolling gets every frame
    if (moving || now - lastDraw > 32) {
      const dt = now - lastDraw;
      lastDraw = now; lastP = p;
      frame(now);
      // if frames keep coming in slow, drop the resolution once
      if (dt < 200) { frames++; if (dt > 40) slow++; }
      if (frames >= 40) {
        if (slow > 24 && dpr > 1) { dpr = 1; renderer.setPixelRatio(1); resize(); }
        frames = 0; slow = 0;
      }
    }
    if (!opts.reduced) raf = requestAnimationFrame(loop);
  }
  document.addEventListener("visibilitychange", () => { if (!document.hidden && visible && !raf) loop(0); });
  if (opts.reduced) window.addEventListener("scroll", () => frame(0), { passive: true });
  loop(0);
  return { render: () => frame(performance.now()) };
}

declare global {
  interface Window { GarudaHero: { mount: typeof mountQueenHero } }
}
window.GarudaHero = { mount: mountQueenHero };
window.dispatchEvent(new Event("garuda-hero-ready"));
