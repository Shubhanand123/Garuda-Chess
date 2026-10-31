import * as React from "react";
import { createRoot, type Root } from "react-dom/client";
import { GlassIconReveal } from "@/components/glass-reveal";
import { ClothingRailHero, type RailGarment } from "@/components/clothing-rail-hero";
import HalftoneWordmarkFooter from "@/components/halftone-wordmark-footer";
import { ContainerScroll } from "@/components/container-scroll-animation";

/** Garuda wing mark, used as the glass-reveal logo (shape only). */
export const GARUDA_MARK = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="9 17 42 40"><path d="M12 38c8-2 14-8 18-18 1 8 4 13 9 15-4 1-6 3-7 6 6-1 11 1 16 6-9 0-15 3-20 8 1-7-2-12-16-17z"/></svg>`;

type BotItem = RailGarment & { level: number };
interface BotRailProps {
  items: BotItem[];
  eyebrow: string;
  heading: string;
  caption: string;
  onSelect: (level: number) => void;
  onPlay: (level: number) => void;
  playLabel: (level: number | null) => string;
}

function BotRail({ items, eyebrow, heading, caption, onSelect, onPlay, playLabel }: BotRailProps) {
  const [open, setOpen] = React.useState<number | null>(null);
  const [chosen, setChosen] = React.useState<number | null>(null);
  return (
    <ClothingRailHero
      items={items}
      eyebrow={eyebrow}
      heading={heading}
      caption={caption}
      hanger="wood"
      autoplay
      interval={2600}
      labels={{ front: "Front", back: "Back", close: "Close", show: "Show" }}
      onOpenChange={(i) => {
        setOpen(i);
        if (i != null) {
          setChosen(items[i].level);
          onSelect(items[i].level);
        }
      }}
      action={
        <button
          type="button"
          onClick={() => onPlay(chosen ?? (open != null ? items[open].level : 3))}
          className="mt-3 inline-flex min-h-11 items-center gap-2 rounded-xl px-5 text-sm font-bold text-[#1a0f05] shadow-lg transition hover:brightness-110"
          style={{ background: "linear-gradient(135deg,#f3a43b,#e2603a)" }}
        >
          {playLabel(chosen)}
        </button>
      }
    />
  );
}


interface HeroProps {
  kicker: string;
  title: string;
  accent: string;
  subtitle: string;
  primary: string;
  secondary: string;
  onPrimary: () => void;
  onSecondary: () => void;
  onBoard: (el: HTMLDivElement) => void;
}

function Hero({ kicker, title, accent, subtitle, primary, secondary, onPrimary, onSecondary, onBoard }: HeroProps) {
  const ref = React.useCallback((el: HTMLDivElement | null) => { if (el) onBoard(el); }, [onBoard]);
  return (
    <div className="flex flex-col overflow-hidden">
      <ContainerScroll
        titleComponent={
          <div className="flex flex-col items-center gap-4 px-4">
            <span className="font-mono text-[11px] uppercase tracking-[0.3em] text-muted-foreground">{kicker}</span>
            <h1 className="text-balance text-4xl font-semibold leading-tight text-foreground md:text-5xl">
              {title}
              <br />
              <span className="mt-1 block bg-gradient-to-r from-[#f3a43b] to-[#e2603a] bg-clip-text text-5xl font-extrabold leading-none tracking-tight text-transparent md:text-[6.5rem]">{accent}</span>
            </h1>
            <p className="max-w-xl text-pretty text-base text-muted-foreground md:text-lg">{subtitle}</p>
            <div className="mb-16 flex flex-wrap justify-center gap-3 md:mb-24">
              <button type="button" onClick={onPrimary} className="inline-flex min-h-12 items-center rounded-xl px-6 text-base font-bold text-[#1a0f05] shadow-lg transition hover:brightness-110" style={{ background: "linear-gradient(135deg,#f3a43b,#e2603a)" }}>{primary}</button>
              <button type="button" onClick={onSecondary} className="inline-flex min-h-12 items-center rounded-xl border border-border px-6 text-base font-semibold text-foreground transition hover:border-muted-foreground">{secondary}</button>
            </div>
          </div>
        }
      >
        <div ref={ref} className="h-full w-full" />
      </ContainerScroll>
    </div>
  );
}

const roots = new Map<Element, Root>();
function render(el: Element, node: React.ReactNode) {
  let r = roots.get(el);
  if (!r) {
    r = createRoot(el);
    roots.set(el, r);
  }
  r.render(node);
}

declare global {
  interface Window {
    GarudaUI: Record<string, unknown>;
  }
}

window.GarudaUI = {
  mark: GARUDA_MARK,
  glass(el: Element, props: Record<string, unknown>) {
    render(el, <GlassIconReveal logo={GARUDA_MARK} label="Garuda" {...(props as object)} />);
  },
  rail(el: Element, props: BotRailProps) {
    render(el, <BotRail {...props} />);
  },
  hero(el: Element, props: HeroProps) {
    render(el, <Hero {...props} />);
  },
  footer(el: Element, props: React.ComponentProps<typeof HalftoneWordmarkFooter>) {
    render(el, <HalftoneWordmarkFooter {...props} />);
  },
  unmount(el: Element) {
    roots.get(el)?.unmount();
    roots.delete(el);
  },
};
window.dispatchEvent(new Event("garuda-ui-ready"));
