import { Outlet, useLocation } from "react-router";
import BottomNav from "../components/BottomNav";

export default function MobileLayout() {
  const { pathname } = useLocation();
  const isReader = pathname.startsWith("/feeds/");

  return (
    <div className="flex h-screen flex-col lg:hidden">
      <main className="min-h-0 flex-1 overflow-y-auto">
        <Outlet />
      </main>
      {!isReader && <BottomNav />}
    </div>
  );
}
