import type { Location } from "react-router";
import { Route, Routes, useLocation, useNavigate } from "react-router";
import { XMarkIcon } from "@heroicons/react/24/outline";
import Shell from "./components/Shell";
import Login from "./pages/Login";
import Overview from "./pages/Overview";
import Feeds from "./pages/Feeds";
import Saved from "./pages/Saved";
import Settings from "./pages/Settings";
import Sources from "./pages/Sources";
import Help from "./pages/Help";
import { useTheme } from "./theme/useTheme";

function ThemeManager() {
  useTheme();
  return null;
}

function SourcesSlideOver({ onClose }: { onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 hidden lg:block">
      <button
        type="button"
        aria-label="Close sources panel"
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/50"
      />
      <aside className="absolute inset-y-0 right-0 flex w-full max-w-md flex-col border-l border-app-border bg-app-bg shadow-2xl">
        <div className="flex items-center justify-end p-2">
          <button
            type="button"
            onClick={onClose}
            aria-label="Close sources panel"
            className="rounded-md p-1.5 text-app-text-muted hover:bg-app-hover hover:text-app-text"
          >
            <XMarkIcon className="h-5 w-5" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-hidden">
          <Sources />
        </div>
      </aside>
    </div>
  );
}

export default function App() {
  const location = useLocation();
  const navigate = useNavigate();
  const background = (location.state as { backgroundLocation?: Location } | null)?.backgroundLocation;
  const showSourcesOverlay = background !== undefined && location.pathname === "/sources";

  const closeSources = () => {
    navigate(background ?? "/");
  };

  return (
    <>
      <ThemeManager />
      <Routes location={background ?? location}>
        <Route path="/login" element={<Login />} />
        <Route path="/" element={<Shell />}>
          <Route index element={<Overview />} />
          <Route path="feeds/*" element={<Feeds />} />
          <Route path="saved" element={<Saved />} />
          <Route path="settings" element={<Settings />} />
          <Route path="sources" element={<Sources />} />
          <Route path="help" element={<Help />} />
        </Route>
      </Routes>
      {showSourcesOverlay && <SourcesSlideOver onClose={closeSources} />}
    </>
  );
}
