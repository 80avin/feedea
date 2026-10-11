import { useEffect, useRef } from "react";
import { useLocation, useNavigationType } from "react-router";

const positions = new Map<string, number>();

export function visibleScrollRoot(): HTMLElement | null {
  const roots = document.querySelectorAll("main[data-scroll-root]");
  for (const root of roots) {
    if (root.clientHeight > 0) return root as HTMLElement;
  }
  return null;
}

/**
 * Mount once in Shell (which never unmounts): records the visible scroller
 * offset under the outgoing pathname. Lives here — not in page cleanup —
 * so StrictMode remounts can't overwrite a good saved value with a
 * transient clamped one.
 */
export function useSaveScrollRoot() {
  const { pathname } = useLocation();
  const prev = useRef(pathname);
  useEffect(() => {
    const el = visibleScrollRoot();
    if (el) positions.set(prev.current, el.scrollTop);
    prev.current = pathname;
  }, [pathname]);
}

/**
 * Browser-style restoration for the app's inner scroll container. The
 * browser only persists document scroll (always 0 here — the app scrolls
 * inside layout <main>), so restore that offset ourselves. POP restores the
 * saved offset (again once `ready`, for async content); PUSH starts at top.
 * Never yanks after the user has scrolled.
 */
export function useRestoreScrollRoot(ready = true) {
  const { pathname } = useLocation();
  const navType = useNavigationType();
  const touched = useRef(false);

  useEffect(() => {
    const el = visibleScrollRoot();
    if (!el) return;
    touched.current = false;
    const onScroll = () => {
      touched.current = true;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    if (navType !== "POP") {
      el.scrollTop = 0;
    } else {
      const saved = positions.get(pathname);
      if (saved != null) el.scrollTop = saved;
    }
    return () => el.removeEventListener("scroll", onScroll);
  }, [pathname, navType]);

  useEffect(() => {
    if (navType !== "POP" || !ready || touched.current) return;
    const saved = positions.get(pathname);
    const el = visibleScrollRoot();
    if (el && saved != null) el.scrollTop = saved;
  }, [pathname, navType, ready]);
}
