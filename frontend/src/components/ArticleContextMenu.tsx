import { useLayoutEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import {
  ArrowTopRightOnSquareIcon,
  BookmarkIcon,
  CheckIcon,
  ClipboardDocumentIcon,
  EyeIcon,
  EyeSlashIcon,
  FolderIcon,
  ShareIcon,
  XMarkIcon,
} from "@heroicons/react/24/outline";
import type { Headline } from "../api/types";
import { encodeId } from "../api/client";
import { useFeedRead, useMarkRead, useSaveArticle, useUnsaveArticle } from "../state/hooks";
import type { ContextMenuPosition } from "../hooks/useContextMenu";

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

async function shareArticle(title: string | null, url: string | null): Promise<"shared" | "copied"> {
  if (!url) return "copied";
  if (navigator.share) {
    try {
      await navigator.share({ title: title ?? "", url });
      return "shared";
    } catch {
      // dismissed — do nothing
    }
  }
  return (await copyText(url)) ? "copied" : "copied";
}

export default function ArticleContextMenu({
  article,
  position,
  menuRef,
  onClose,
}: {
  article: Headline;
  position: ContextMenuPosition;
  menuRef: React.RefObject<HTMLDivElement | null>;
  onClose: () => void;
}) {
  const itemRef = useRef<HTMLDivElement>(null);
  const [clamped, setClamped] = useState(position);
  const markRead = useMarkRead();
  const save = useSaveArticle();
  const unsave = useUnsaveArticle();
  const feedRead = useFeedRead();
  const navigate = useNavigate();

  useLayoutEffect(() => {
    const el = itemRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const maxX = Math.max(4, window.innerWidth - rect.width - 4);
    const maxY = Math.max(4, window.innerHeight - rect.height - 4);
    setClamped({ x: Math.min(Math.max(position.x, 4), maxX), y: Math.min(Math.max(position.y, 4), maxY) });
  }, [position]);

  const itemClass =
    "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm text-app-text-2 hover:bg-app-hover hover:text-app-text disabled:cursor-not-allowed disabled:opacity-50";

  const actions = [
    {
      label: "Open in browser",
      icon: ArrowTopRightOnSquareIcon,
      disabled: !article.url,
      onPress: () => {
        if (article.url) window.open(article.url, "_blank", "noopener");
        onClose();
      },
    },
    {
      label: "Copy URL",
      icon: ClipboardDocumentIcon,
      disabled: !article.url,
      onPress: async () => {
        await copyText(article.url ?? "");
        onClose();
      },
    },
    {
      label: "Copy title",
      icon: ClipboardDocumentIcon,
      disabled: !article.title,
      onPress: async () => {
        await copyText(article.title ?? "");
        onClose();
      },
    },
    {
      label: "Share",
      icon: ShareIcon,
      disabled: !article.url,
      onPress: async () => {
        await shareArticle(article.title, article.url);
        onClose();
      },
    },
    { separator: true },
    {
      label: article.unread ? "Mark read" : "Mark unread",
      icon: article.unread ? EyeIcon : EyeSlashIcon,
      onPress: () => {
        markRead.mutate({ id: article.id, read: article.unread });
        onClose();
      },
    },
    {
      label: article.marked ? "Unsave" : "Save",
      icon: article.marked ? XMarkIcon : BookmarkIcon,
      onPress: () => {
        if (article.marked) {
          unsave.mutate({ id: article.id });
        } else {
          save.mutate({ id: article.id, note: undefined, tags: [] });
        }
        onClose();
      },
    },
    { separator: true },
    {
      label: "Open feed",
      icon: FolderIcon,
      onPress: () => {
        navigate(`/feeds?feed=${encodeId(article.feed_id)}`);
        onClose();
      },
    },
    {
      label: "Mark all read",
      icon: CheckIcon,
      onPress: () => {
        feedRead.mutate({ id: article.feed_id });
        onClose();
      },
    },
  ] as const;

  return (
    <div
      ref={(node) => {
        menuRef.current = node;
        itemRef.current = node;
      }}
      data-context-menu
      role="menu"
      className="fixed z-50 min-w-48 rounded-md border border-app-border-strong bg-app-bg py-1 shadow-xl"
      style={{ left: clamped.x, top: clamped.y }}
    >
      {actions.map((action, index) =>
        "separator" in action ? (
          <div key={index} role="separator" className="my-1 border-t border-app-border/60" />
        ) : (
          <button
            key={index}
            type="button"
            role="menuitem"
            className={itemClass}
            disabled={"disabled" in action && action.disabled}
            onClick={action.onPress}
          >
            <action.icon className="h-4 w-4 shrink-0" />
            {action.label}
          </button>
        ),
      )}
    </div>
  );
}