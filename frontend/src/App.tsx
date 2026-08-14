import { Route, Routes } from "react-router";
import Shell from "./components/Shell";
import Home from "./pages/Home";
import Login from "./pages/Login";

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route path="/" element={<Shell />}>
        <Route index element={<Home />} />
      </Route>
    </Routes>
  );
}
