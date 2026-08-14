import { useSelectedArticleId } from "../hooks/useSelectedArticleId";

export default function ReaderPanel() {
  const id = useSelectedArticleId();

  return (
    <div className="flex h-full flex-col p-4">
      <h2 className="text-lg font-semibold">Reader</h2>
      <p className="text-sm text-zinc-400">{id ? `Reading article ${id}.` : "No article selected."}</p>
      <p className="mt-1 text-sm text-zinc-600">Reader coming in Phase 5.</p>
    </div>
  );
}
