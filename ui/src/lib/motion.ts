// Animation speed and "calm" motion for everything, using the Web
// Animations API: each CSS animation and transition is caught as it starts.
import type { FeelPrefs } from "./theme";

export function installMotion(feel: () => FeelPrefs) {
  if (typeof document === "undefined") return () => {};
  const tune = (e: Event) => {
    const f = feel();
    if (f.motion === "off" || (f.motion === "full" && f.speed === 1)) return;
    const el = e.target as Element | null;
    if (!el?.getAnimations) return;
    const name = (e as AnimationEvent).animationName ?? (e as TransitionEvent).propertyName;
    for (const a of el.getAnimations()) {
      const id = (a as CSSAnimation).animationName ?? (a as CSSTransition).transitionProperty;
      if (id !== name) continue;
      // Calm: slower, and endless decorative loops hold still.
      const rate = f.motion === "calm" ? f.speed * 0.6 : f.speed;
      a.playbackRate = rate;
      if (f.motion === "calm" && a.effect?.getComputedTiming().iterations === Infinity) a.pause();
    }
  };
  document.addEventListener("animationstart", tune, true);
  document.addEventListener("transitionrun", tune, true);
  return () => {
    document.removeEventListener("animationstart", tune, true);
    document.removeEventListener("transitionrun", tune, true);
  };
}
