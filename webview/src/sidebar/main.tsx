import "@vscode/codicons/dist/codicon.css";
import "../tokens.css";
import "../style.css";

import { createRoot } from "react-dom/client";

import { ErrorBoundary } from "../ErrorBoundary.tsx";
import { Sidebar } from "./Sidebar.tsx";

// The STRUCTURE view's own bundle (docs/16-ui-redesign/06-sidebar.md §10): no React Flow, no Monaco, no Shiki, so the side bar opens at once.
createRoot(document.getElementById("root")!).render(
  <ErrorBoundary>
    <Sidebar />
  </ErrorBoundary>,
);
