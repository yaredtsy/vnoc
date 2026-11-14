import { Button } from "@/components/ui/button.tsx";
import { request } from "../host.ts";
import { refresh } from "../state/data.ts";
import type { SidebarBody } from "./sidebarBody.ts";

/** The four states of 06 §8 that have no rows: no folder, reading the code, no supported file, an error. They replace the native view's `viewsWelcome`. */
export function States({ body, message }: { body: Exclude<SidebarBody, "ready">; message: string }) {
  switch (body) {
    case "noFolder":
      return (
        <Centered>
          <p className="text-muted-foreground">Open a folder to see its code.</p>
          <Button size="sm" onClick={() => void request("command", { name: "openFolder" }).catch(() => {})}>
            Open Folder
          </Button>
        </Centered>
      );
    case "loading":
      return (
        <div role="status" aria-label="Reading the code" className="flex-1 py-1">
          {[60, 45, 70, 40, 55].map((width, i) => (
            <div key={i} className="flex h-[22px] items-center gap-2 px-2" style={{ paddingLeft: 8 + (i % 2) * 16 }}>
              <span className="size-4 flex-none rounded-sm bg-muted" />
              <span className="h-2.5 rounded-sm motion-safe:animate-pulse bg-muted" style={{ width: `${width}%` }} />
            </div>
          ))}
        </div>
      );
    case "empty":
      return (
        <Centered>
          <p className="text-muted-foreground">No Python files here yet.</p>
        </Centered>
      );
    case "error":
      return (
        <Centered>
          <p className="whitespace-pre-wrap text-destructive">{message || "V-NOC could not read this project."}</p>
          <div className="flex gap-2">
            <Button
              size="sm"
              onClick={() => {
                void request("command", { name: "refreshStructure" }).catch(() => {});
                refresh();
              }}
            >
              Try Again
            </Button>
            <Button size="sm" variant="outline" onClick={() => void request("showLog").catch(() => {})}>
              Show Log
            </Button>
          </div>
        </Centered>
      );
  }
}

function Centered({ children }: { children: React.ReactNode }) {
  return <div className="flex flex-1 flex-col items-start gap-3 p-4">{children}</div>;
}
