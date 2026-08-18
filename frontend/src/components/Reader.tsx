import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { ChevronLeftIcon } from "@heroicons/react/24/outline";
import ArticleHtml from "./ArticleHtml";
import Feedback from "./Feedback";
import ReaderActions from "./ReaderActions";
import SaveDialog from "./SaveDialog";
import { useSelectedArticleId } from "../hooks/useSelectedArticleId";
import { useArticle, useMarkRead } from "../state/hooks";
import { formatAge } from "../utils/format";
import FeedNameLink from "./FeedNameLink";

const autoMarkedReadIds = new Set<string>();

function hostOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export default function Reader() {
  const id = useSelectedArticleId();
  const navigate = useNavigate();
  const { data, isLoading, isError, error, refetch } = useArticle(id ?? "");
  const markRead = useMarkRead();
  const [saveOpen, setSaveOpen] = useState(false);

  useEffect(() => {
    if (id && data && data.unread && !autoMarkedReadIds.has(id)) {
      autoMarkedReadIds.add(id);
      markRead.mutate({ id, read: true });
    }
  }, [id, data, markRead]);

  const goBack = () => {
    if (window.history.state?.idx && window.history.state.idx > 0) {
      navigate(-1);
    } else {
      navigate("/feeds");
    }
  };

  if (!id) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-app-text-faint">
        Select an article to read it here.
      </div>
    );
  }

  if (isLoading || isError) {
    return (
      <div className="p-4">
        <Feedback loading={isLoading} error={error} loadingLabel="Loading article…" onRetry={() => refetch()} />
      </div>
    );
  }

  if (!data) return null;

  return (
    <div className="flex h-full flex-col pb-20 lg:pb-0">
      <div className="p-4 pb-0">
        <div className="flex items-start gap-2">
          <button
            type="button"
            onClick={goBack}
            aria-label="Back to feeds"
            className="rounded-md p-1 text-app-text-muted hover:bg-app-hover hover:text-app-text lg:hidden"
          >
            <ChevronLeftIcon className="h-6 w-6" />
          </button>
          {data.title && <h1 className="text-xl font-semibold leading-snug text-app-text">{data.title}</h1>}
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-app-text-muted" data-reader-meta>
          {data.feed_title && (
            <FeedNameLink
              feedId={data.feed_id}
              title={data.feed_title}
              className="font-medium text-app-text-2 hover:text-accent hover:underline"
            />
          )}
          {data.author && <span>by {data.author}</span>}
          <span>{formatAge(data.date)}</span>
          <span>{new Date(data.date).toLocaleDateString()}</span>
          {data.url && (
            <a
              href={data.url}
              target="_blank"
              rel="noopener noreferrer"
              className="truncate text-accent hover:text-accent-soft-foreground"
            >
              {hostOf(data.url)}
            </a>
          )}
        </div>
        <div className="mt-3 hidden lg:block">
          <ReaderActions article={data} onSaveClick={() => setSaveOpen(true)} />
        </div>
      </div>

      <div className="mt-4 min-h-0 flex-1 px-4 pb-4">
        <ArticleHtml html={data.html} summary={data.summary} plainText={data.plain_text} />
        {data.note && (
          <div className="mt-6 rounded-lg border border-accent/30 bg-accent-soft p-4" data-reader-note>
            <p className="text-xs font-semibold uppercase tracking-wider text-accent-soft-foreground">Note</p>
            <p className="mt-1.5 whitespace-pre-wrap text-sm leading-relaxed text-app-text-2">{data.note}</p>
          </div>
        )}
      </div>

      <div
        className="fixed inset-x-0 bottom-0 z-20 border-t border-app-border bg-app-bg/95 px-4 py-1.5 backdrop-blur lg:hidden"
        data-reader-bar
      >
        <ReaderActions article={data} onSaveClick={() => setSaveOpen(true)} layout="bar" />
      </div>

      <SaveDialog open={saveOpen} articleId={data.id} onClose={() => setSaveOpen(false)} />
    </div>
  );
}
