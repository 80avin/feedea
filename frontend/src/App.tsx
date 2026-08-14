import { Route, Routes } from "react-router";
import Shell from "./components/Shell";
import Login from "./pages/Login";
import Overview from "./pages/Overview";
import Feeds from "./pages/Feeds";
import Saved from "./pages/Saved";
import Settings from "./pages/Settings";
import Sources from "./pages/Sources";
import Help from "./pages/Help";

export default function App() {
  return (
    <Routes>
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
  );
}
