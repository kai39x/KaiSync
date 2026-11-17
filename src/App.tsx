import { useState } from "react";
import { MotionConfig } from "motion/react";
import { Shell, type Page } from "./components/shell/Shell";
import { Games } from "./screens/Games";
import { Home } from "./screens/Home";
import { Settings } from "./screens/Settings";
import { StoreProvider } from "./state/store";

export default function App() {
  const [page, setPage] = useState<Page>("home");
  return (
    <MotionConfig reducedMotion="user">
      <StoreProvider>
        <Shell page={page} onNavigate={setPage}>
          {page === "home" ? <Home /> : page === "games" ? <Games /> : <Settings />}
        </Shell>
      </StoreProvider>
    </MotionConfig>
  );
}
