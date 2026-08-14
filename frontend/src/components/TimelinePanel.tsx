import { Link } from "react-router";
import type { Headline } from "../api/types";
import ArticleListItem from "./ArticleListItem";

export default function TimelinePanel({ articles }: { articles: Headline[] }) {
  return (
    <div className="flex h-full flex-col">
      <ul className="divide-y divide-zinc-800/60 px-4">
        {articles.map((item) => (
          <li key={item.id}>
            <Link to={`/feeds/${item.id}`} className="block">
              <ArticleListItem item={item} />
            </Link>
          </li>
        ))}
        {articles.length === 0 && <li className="py-8 text-center text-sm text-zinc-600">No articles.</li>}
      </ul>
    </div>
  );
}
