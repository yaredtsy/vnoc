import "@xyflow/react/dist/style.css";
import "./style.css";

import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";
import { DocsApp } from "./docs/DocsApp.tsx";
import { ErrorBoundary } from "./ErrorBoundary.tsx";

// One bundle, two modes: the host says which when it creates the webview (`data-mode` on the root).
const root = document.getElementById("root")!;
createRoot(root).render(<ErrorBoundary>{root.dataset.mode === "docs" ? <DocsApp /> : <App />}</ErrorBoundary>);
