import { useCallback, useEffect, useRef, useState } from "react";

export interface ContextMenuPosition {
  x: number;
  y: number;
}

const LONG_PRESS_MS = 500;
const MOVE_TOLERANCE = 12;

export function useContextMenu() {
  const [position, setPosition] = useState<ContextMenuPosition | null>(null);
  const timerRef = useRef<number | null>(null);
  const suppressClickRef = useRef(false);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const pressOriginRef = useRef<{ x: number; y: number } | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const open = useCallback(
    (x: number, y: number) => {
      clearTimer();
      setPosition({ x, y });
    },
    [clearTimer],
  );

  const close = useCallback(() => setPosition(null), []);

  useEffect(() => {
    if (!position) return;
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node | null;
      if (target && menuRef.current?.contains(target)) return;
      close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    const onScroll = () => close();
    const onResize = () => close();
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  }, [position, close]);

  const triggerProps = {
    onContextMenu: (e: React.MouseEvent) => {
      e.preventDefault();
      open(e.clientX, e.clientY);
    },
    onPointerDown: (e: React.PointerEvent) => {
      if (e.pointerType !== "touch") return;
      suppressClickRef.current = false;
      pressOriginRef.current = { x: e.clientX, y: e.clientY };
      clearTimer();
      timerRef.current = window.setTimeout(() => {
        suppressClickRef.current = true;
        open(e.clientX, e.clientY);
      }, LONG_PRESS_MS);
    },
    onPointerMove: (e: React.PointerEvent) => {
      if (e.pointerType !== "touch") return;
      const origin = pressOriginRef.current;
      if (origin && Math.hypot(e.clientX - origin.x, e.clientY - origin.y) > MOVE_TOLERANCE) {
        clearTimer();
      }
    },
    onPointerUp: (e: React.PointerEvent) => {
      if (e.pointerType === "touch") clearTimer();
    },
    onPointerCancel: () => clearTimer(),
    onClick: (e: React.MouseEvent) => {
      if (suppressClickRef.current) {
        e.preventDefault();
        e.stopPropagation();
        suppressClickRef.current = false;
      }
    },
  };

  useEffect(() => clearTimer, [clearTimer]);

  return { position, close, menuRef, triggerProps };
}
