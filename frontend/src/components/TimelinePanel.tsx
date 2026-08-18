import { useState } from "react";
import { useNavigate } from "react-router";
import type { Headline } from "../api/types";
import { encodeId } from "../api/client";
import { formatAge } from "../utils/format";
import FeedAvatar from "./FeedAvatar";
import ArticleContextMenu from "./ArticleContextMenu";
import { useContextMenu } from "../hooks/useContextMenu";
import { useArticlePath } from "../hooks/useArticlePath";
import FeedNameLink from "./FeedNameLink";

function TimelineItem({ item }: { item: Headline }) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const { position, close, menuRef, triggerProps } = useContextMenu();
  const articlePathFn = useArticlePath();
  const navigate = useNavigate();

  const openArticle = () => navigate(articlePathFn(item.id));
  const openArticleNewTab = (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault();
      window.open(articlePathFn(item.id), "_blank", "noopener");
    }
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openArticle();
    }
  };

  return (
    <>
      <div
        role="link"
        tabIndex={0}
        className="block cursor-pointer px-4 py-2 hover:bg-app-surface/60"
        {...triggerProps}
        onClick={(e) => {
          if (triggerProps.onClick) triggerProps.onClick(e);
          if (!e.isPropagationStopped()) openArticle();
        }}
        onAuxClick={openArticleNewTab}
        onKeyDown={onKeyDown}
      >
        <div className="flex items-start gap-3">
          <FeedAvatar feedId={item.feed_id} title={item.feed_title} className="h-8 w-8" />
          <div className="min-w-0 flex-1">
            {item.title && (
              <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>
                {item.title}
              </p>
            )}
            <p className="truncate text-xs text-app-text-faint">
              {item.date ? formatAge(item.date) : ""}
              {item.date && item.feed_title ? " · " : ""}
              {item.feed_title && (
                <FeedNameLink
                  feedId={item.feed_id}
                  title={item.feed_title}
                  className="text-app-text-faint hover:text-accent hover:underline"
                />
              )}
            </p>
          </div>
          {item.thumbnail_url && !thumbFailed && (
            <img
              src={`/api/thumbnail/${encodeId(item.id)}`}
              alt=""
              loading="lazy"
              onError={() => setThumbFailed(true)}
              className="h-12 w-16 shrink-0 rounded-md object-cover"
            />
          )}
        </div>
      </div>
      {position && (
        <ArticleContextMenu article={item} position={position} menuRef={menuRef} onClose={close} />
      )}
    </>
  );
}

export default function TimelinePanel({
  articles,
  bottomRef,
}: {
  articles: Headline[];
  bottomRef?: (node: Element | null) => void;
}) {
  return (
    <div className="flex h-full flex-col">
      <ul className="divide-y divide-app-border/60">
        {articles.map((item) => (
          <li key={item.id}>
            <TimelineItem item={item} />
          </li>
        ))}
        {articles.length === 0 && <li className="py-8 text-center text-sm text-app-text-faint">No articles.</li>}
      </ul>
      <div ref={bottomRef} className="h-px" />
    </div>
  );
}
