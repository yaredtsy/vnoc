import { useCanvasActions } from "../canvas/CanvasActions.ts";
import type { Placed } from "../canvas/layout.ts";
import { KINDS } from "./kinds.ts";

export function NodeHeader({ node }: { node: Placed }) {
  const { toggle, toggleCode } = useCanvasActions();
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
      {node.range && (
        <button className={`code-toggle nodrag nopan${node.codeOpen ? " on" : ""}`} aria-pressed={node.codeOpen} aria-label={`Show the code of ${node.name}`} onClick={() => toggleCode(node.id)}>
          {"</>"}
        </button>
      )}
    </div>
  );
}
