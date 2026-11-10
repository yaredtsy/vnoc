import "@xyflow/react/dist/style.css";
import "./style.css";

import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";
import { DocsApp } from "./docs/DocsApp.tsx";

// One bundle, two modes: the host says which when it creates the webview (`data-mode` on the root).
const root = document.getElementById("root")!;
createRoot(root).render(root.dataset.mode === "docs" ? <DocsApp /> : <App />);
