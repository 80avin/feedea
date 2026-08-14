import { Outlet } from "react-router";
import clsx from "clsx";
import Sidebar from "../components/Sidebar";
import ReaderPanel from "../components/ReaderPanel";
import { useSelectedArticleId } from "../hooks/useSelectedArticleId";

export default function DesktopLayout() {
  const articleId = useSelectedArticleId();
  const showReader = !!articleId;

  return (
    <div
      className={clsx(
        "hidden h-screen overflow-hidden lg:grid",
        showReader
          ? "lg:grid-cols-[240px_minmax(0,1fr)_minmax(0,1fr)]"
          : "lg:grid-cols-[240px_minmax(0,1fr)]",
      )}
    >
      <Sidebar />
      <main className={clsx("min-h-0 overflow-y-auto", showReader && "border-r border-zinc-800")}>
        <Outlet />
      </main>
      {showReader && (
        <aside className="min-h-0 overflow-y-auto">
          <ReaderPanel />
        </aside>
      )}
    </div>
  );
}
