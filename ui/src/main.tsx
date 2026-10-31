import * as React from "react";
import { createRoot, type Root } from "react-dom/client";
import { GlassIconReveal } from "@/components/glass-reveal";
import { ClothingRailHero, type RailGarment } from "@/components/clothing-rail-hero";
import HalftoneWordmarkFooter from "@/components/halftone-wordmark-footer";

/** Garuda wing mark, used as the glass-reveal logo (shape only). */
export const GARUDA_MARK = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="6 8 52 50"><path d="M14 40 L11 18 L22 29 L32 12 L42 29 L53 18 L50 40 Z"/><circle cx="11" cy="16" r="3.4"/><circle cx="32" cy="10" r="3.6"/><circle cx="53" cy="16" r="3.4"/><path d="M14 43 H50 V48 H14 Z"/><path d="M9 52 C22 56 40 56 56 48 C50 56 40 58 30 58 C20 58 13 56 9 52 Z"/></svg>`;

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
          style={{ background: "linear-gradient(135deg,#f7d77c,#d99a2b)" }}
        >
          {playLabel(chosen)}
        </button>
      }
    />
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
  footer(el: Element, props: React.ComponentProps<typeof HalftoneWordmarkFooter>) {
    render(el, <HalftoneWordmarkFooter {...props} />);
  },
  unmount(el: Element) {
    roots.get(el)?.unmount();
    roots.delete(el);
  },
};
window.dispatchEvent(new Event("garuda-ui-ready"));
