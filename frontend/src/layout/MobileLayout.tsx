import { Outlet } from "react-router";
import BottomNav from "../components/BottomNav";
import ReaderPanel from "../components/ReaderPanel";
import { useSelectedArticleId } from "../hooks/useSelectedArticleId";

export default function MobileLayout() {
  const articleId = useSelectedArticleId();
  const isReader = !!articleId;

  return (
    <div className="flex h-screen flex-col lg:hidden">
      <main className="min-h-0 flex-1 overflow-y-auto">
        {isReader ? <ReaderPanel /> : <Outlet />}
      </main>
      {!isReader && <BottomNav />}
    </div>
  );
}
