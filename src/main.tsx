import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

// Vorschau im Browser ohne Tauri: Beispieldaten statt echter Befehle (nur im Entwicklungsmodus)
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  await import("./devmock");
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
