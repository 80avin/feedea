import { useEffect, useRef } from "react";
import type { MouseEvent } from "react";

interface ArticleHtmlProps {
  html: string | null;
  summary: string | null;
  plainText: string | null;
}

function openExternal(target: Element) {
  const anchor = target.closest("a[href]");
  if (!anchor) return;
  const href = anchor.getAttribute("href") ?? "";
  if (/^https?:\/\//i.test(href)) {
    window.open(href, "_blank", "noopener");
  }
}

function showTargetOnHover(target: Element) {
  const anchor = target.closest("a[href]");
  if (anchor) {
    const href = anchor.getAttribute("href") ?? "";
    if (/^https?:\/\//i.test(href) && !anchor.getAttribute("title")) {
      anchor.setAttribute("title", href);
    }
  }
  const img = target.closest("img[data-original]");
  if (img && !img.getAttribute("title")) {
    img.setAttribute("title", img.getAttribute("data-original") ?? "");
  }
}

export default function ArticleHtml({ html, summary, plainText }: ArticleHtmlProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const handler = (e: Event) => {
      const target = e.target as HTMLElement;
      if (target instanceof HTMLImageElement) {
        target.style.display = "none";
      }
    };
    el.addEventListener("error", handler, true);
    return () => el.removeEventListener("error", handler, true);
  }, [html]);

  if (!html) {
    const text = summary ?? plainText;
    if (!text) return null;
    return (
      <div className="article-prose" data-reader-html>
        <p className="whitespace-pre-wrap">{text}</p>
      </div>
    );
  }

  const onMouseOver = (e: MouseEvent<HTMLDivElement>) => showTargetOnHover(e.target as Element);

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const target = e.target as Element;
    const anchor = target.closest("a[href]");
    if (!anchor) return;
    const href = anchor.getAttribute("href") ?? "";
    if (/^https?:\/\//i.test(href)) {
      e.preventDefault();
      openExternal(target);
    }
  };

  return (
    <div
      ref={containerRef}
      onMouseOver={onMouseOver}
      onClick={onClick}
      className="article-prose"
      data-reader-html
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
