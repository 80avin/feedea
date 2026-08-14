import { useCallback, useEffect, useRef } from "react";

export function useInfiniteScroll(
  onBottom: () => void,
  options?: { rootMargin?: string },
): (node: Element | null) => void {
  const onBottomRef = useRef(onBottom);
  const observerRef = useRef<IntersectionObserver | null>(null);
  const nodeRef = useRef<Element | null>(null);
  const rootMargin = options?.rootMargin ?? "250px";

  useEffect(() => {
    onBottomRef.current = onBottom;
  }, [onBottom]);

  useEffect(() => {
    return () => observerRef.current?.disconnect();
  }, []);

  return useCallback(
    (node: Element | null) => {
      if (nodeRef.current === node) return;
      let observer = observerRef.current;
      if (!observer) {
        observer = new IntersectionObserver(
          (entries) => {
            for (const entry of entries) {
              if (entry.isIntersecting) {
                onBottomRef.current();
              }
            }
          },
          { rootMargin },
        );
        observerRef.current = observer;
      }
      if (nodeRef.current) {
        observer.unobserve(nodeRef.current);
      }
      nodeRef.current = node;
      if (node) {
        observer.observe(node);
      }
    },
    [rootMargin],
  );
}
