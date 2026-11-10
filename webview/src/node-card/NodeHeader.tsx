import { useCanvasActions } from "../canvas/CanvasActions.ts";
import type { Placed } from "../canvas/layout.ts";
import { KINDS } from "./kinds.ts";

export function NodeHeader({ node }: { node: Placed }) {
  const { toggle, toggleCode, showDocs } = useCanvasActions();
  const kind = KINDS[node.kind];
  return (
    <div className="node-header">
      {node.childCount > 0 ? (
        // `nodrag nopan`: React Flow must not start a drag or a pan from a click on the button
        <button
          className={`expand nodrag nopan${node.collapsed ? "" : " open"}`}
          aria-expanded={!node.collapsed}
          aria-label={node.collapsed ? `Expand ${node.name}` : `Collapse ${node.name}`}
          onClick={() => toggle(node.id)}
        >
          ▸
        </button>
      ) : (
        <span className="expand-spacer" />
      )}
      <span className="icon">{kind.icon}</span>
      <span className="label">{kind.title(node.name)}</span>
      {node.collapsed && <span className="badge">+{node.childCount}</span>}
      {node.link && (
        <span className={`link ${node.link}`} title={node.link === "marker" ? "The ID is in the code: the docs follow it anywhere" : "Found by its stored path and name: the docs are lost if it moves"}>
          🔗
        </span>
      )}
      {node.docs > 0 && (
        <button className="docs nodrag nopan" aria-label={`Show the ${node.docs} docs of ${node.name}`} onClick={() => showDocs(node.id)}>
          📄 {node.docs}
        </button>
      )}
      {node.range && (
        <button className={`code-toggle nodrag nopan${node.codeOpen ? " on" : ""}`} aria-pressed={node.codeOpen} aria-label={`Show the code of ${node.name}`} onClick={() => toggleCode(node.id)}>
          {"</>"}
        </button>
      )}
    </div>
  );
}
