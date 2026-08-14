import { useState } from "react";
import type { ComponentType } from "react";
import {
  ArrowTopRightOnSquareIcon,
  BookmarkIcon,
  CheckIcon,
  EyeIcon,
  EyeSlashIcon,
  ShareIcon,
  XMarkIcon,
} from "@heroicons/react/24/outline";
import clsx from "clsx";
import type { ArticleDetail } from "../api/types";
import { useMarkRead, useUnsaveArticle } from "../state/hooks";

interface ReaderActionsProps {
  article: ArticleDetail;
  onSaveClick: () => void;
  layout?: "row" | "bar";
  className?: string;
}

type Icon = ComponentType<{ className?: string }>;

interface Action {
  key: string;
  label: string;
  icon: Icon;
  onPress: () => void;
  disabled?: boolean;
  pending?: boolean;
  primary?: boolean;
  danger?: boolean;
}

export default function ReaderActions({ article, onSaveClick, layout = "row", className }: ReaderActionsProps) {
  const markRead = useMarkRead();
  const unsave = useUnsaveArticle();
  const [copied, setCopied] = useState(false);

  const toggleRead = () => markRead.mutate({ id: article.id, read: article.unread });

  const openArticle = () => {
    if (article.url) {
      window.open(article.url, "_blank", "noopener");
    }
  };

  const share = async () => {
    if (!article.url) return;
    if (navigator.share) {
      try {
        await navigator.share({ title: article.title ?? "", url: article.url });
        return;
      } catch {}
    }
    try {
      await navigator.clipboard.writeText(article.url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {}
  };

  const actions: Action[] = [
    {
      key: "read",
      label: article.unread ? "Read" : "Unread",
      icon: article.unread ? EyeIcon : EyeSlashIcon,
      onPress: toggleRead,
      pending: markRead.isPending,
      primary: article.unread,
    },
    {
      key: "save",
      label: article.marked ? "Saved" : "Save",
      icon: BookmarkIcon,
      onPress: onSaveClick,
      primary: !article.marked,
    },
    {
      key: "open",
      label: "Open",
      icon: ArrowTopRightOnSquareIcon,
      onPress: openArticle,
      disabled: !article.url,
    },
    {
      key: "share",
      label: copied ? "Copied" : "Share",
      icon: copied ? CheckIcon : ShareIcon,
      onPress: share,
    },
  ];
  if (article.marked) {
    actions.push({
      key: "unsave",
      label: "Unsave",
      icon: XMarkIcon,
      onPress: () => unsave.mutate({ id: article.id }),
      pending: unsave.isPending,
      danger: true,
    });
  }

  if (layout === "bar") {
    return (
      <div className={clsx("flex items-stretch justify-between", className)} data-reader-actions>
        {actions.map((action) => (
          <button
            key={action.key}
            type="button"
            onClick={action.onPress}
            disabled={action.disabled || action.pending}
            className={clsx(
              "flex min-w-0 flex-1 flex-col items-center gap-1 py-1.5 text-[11px] font-medium",
              action.danger ? "text-red-500 dark:text-red-400" : action.primary ? "text-app-text" : "text-app-text-muted",
              action.disabled || action.pending ? "cursor-not-allowed opacity-50" : "hover:text-app-text",
            )}
          >
            <action.icon className="h-5 w-5" />
            {action.label}
          </button>
        ))}
      </div>
    );
  }

  return (
    <div className={clsx("flex flex-wrap items-center gap-1.5", className)} data-reader-actions>
      {actions.map((action) => (
        <button
          key={action.key}
          type="button"
          onClick={action.onPress}
          disabled={action.disabled || action.pending}
          className={clsx(
            "inline-flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-xs font-medium transition-colors",
            action.danger
              ? "border-red-600/40 text-red-600 hover:bg-red-500/10 dark:border-red-500/40 dark:text-red-400 dark:hover:bg-red-500/10"
              : action.primary
                ? "border-accent/40 bg-accent-soft text-accent-soft-foreground hover:bg-accent-soft-hover"
                : "border-app-border text-app-text-2 hover:bg-app-hover/60 hover:text-app-text",
            action.disabled || action.pending ? "cursor-not-allowed opacity-50" : "",
          )}
        >
          <action.icon className="h-3.5 w-3.5" />
          {action.label}
        </button>
      ))}
    </div>
  );
}
