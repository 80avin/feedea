interface FeedbackProps {
  loading: boolean;
  error: unknown;
  loadingLabel?: string;
  onRetry?: () => void;
}

export function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return "Something went wrong.";
}

export function Skeleton({ className }: { className?: string }) {
  return <div className={`animate-pulse rounded-md bg-zinc-800 ${className ?? ""}`} />;
}

export function ErrorState({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  return (
    <div className="flex flex-col items-center gap-3 rounded-lg border border-zinc-800 p-6 text-center">
      <p className="text-sm text-red-400">{formatError(error)}</p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="rounded-md bg-zinc-800 px-3 py-1.5 text-sm font-medium text-zinc-100 hover:bg-zinc-700"
        >
          Retry
        </button>
      )}
    </div>
  );
}

export function LoadingState({ label }: { label?: string }) {
  return (
    <div className="flex flex-col gap-3">
      {Array.from({ length: 3 }).map((_, i) => (
        <Skeleton key={i} className="h-16 w-full" />
      ))}
      {label && <p className="text-sm text-zinc-500">{label}</p>}
    </div>
  );
}

export default function Feedback({ loading, error, loadingLabel, onRetry }: FeedbackProps) {
  if (loading) {
    return <LoadingState label={loadingLabel} />;
  }
  if (error) {
    return <ErrorState error={error} onRetry={onRetry} />;
  }
  return null;
}
