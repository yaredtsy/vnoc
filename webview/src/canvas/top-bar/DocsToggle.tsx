import { BarButton } from "@/components/BarButton.tsx";
import { setDocsOpen, useView } from "@/state/view.ts";

/** Shows or hides the docs panel on the right (pressed while it is open). */
export function DocsToggle() {
  const open = useView((v) => v.docsOpen);
  return <BarButton icon="info" label="Docs of the selected node" aria-pressed={open} onClick={() => setDocsOpen(!open)} />;
}
