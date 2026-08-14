import { useState } from "react";
import { useNavigate } from "react-router";
import { ChevronLeftIcon } from "@heroicons/react/24/outline";
import ArticleHtml from "./ArticleHtml";
import Feedback from "./Feedback";
import ReaderActions from "./ReaderActions";
import SaveDialog from "./SaveDialog";
import { useSelectedArticleId } from "../hooks/useSelectedArticleId";
import { useArticle } from "../state/hooks";
import { formatAge } from "../utils/format";

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
  const [saveOpen, setSaveOpen] = useState(false);

  const goBack = () => {
    if (window.history.state?.idx && window.history.state.idx > 0) {
      navigate(-1);
    } else {
      navigate("/feeds");
    }
  };

  if (!id) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-zinc-500">
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
            className="rounded-md p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 lg:hidden"
          >
            <ChevronLeftIcon className="h-6 w-6" />
          </button>
          {data.title && <h1 className="text-xl font-semibold leading-snug text-zinc-100">{data.title}</h1>}
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-400" data-reader-meta>
          {data.feed_title && <span className="font-medium text-zinc-300">{data.feed_title}</span>}
          {data.author && <span>by {data.author}</span>}
          <span>{formatAge(data.date)}</span>
          <span>{new Date(data.date).toLocaleDateString()}</span>
          {data.url && (
            <a
              href={data.url}
              target="_blank"
              rel="noopener noreferrer"
              className="truncate text-sky-400 hover:text-sky-300"
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
      </div>

      <div
        className="fixed inset-x-0 bottom-0 z-20 border-t border-zinc-800 bg-zinc-950/95 px-4 py-1.5 backdrop-blur lg:hidden"
        data-reader-bar
      >
        <ReaderActions article={data} onSaveClick={() => setSaveOpen(true)} layout="bar" />
      </div>

      <SaveDialog open={saveOpen} articleId={data.id} onClose={() => setSaveOpen(false)} />
    </div>
  );
}
