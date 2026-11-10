import "@xyflow/react/dist/style.css";
import "./style.css";

import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";

createRoot(document.getElementById("root")!).render(<App />);
