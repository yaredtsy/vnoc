import { Button } from "@/components/ui/button.tsx";
import { request } from "@/host.ts";
import { refresh } from "@/state/data.ts";

/** What the page says instead of a canvas, when nothing is drawn yet: the first read of the code, or why it failed. Never shown beside cards. */
export function CanvasMessage({ status, message }: { status: "loading" | "error"; message: string }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-sm">
      {status === "loading" ? (
        <p className="text-muted-foreground">Reading the code…</p>
      ) : (
        <>
          <p className="max-w-xl whitespace-pre-wrap text-destructive">{message}</p>
          <div className="flex gap-2">
            <Button size="sm" onClick={() => refresh()}>
              Try Again
            </Button>
            <Button size="sm" variant="outline" onClick={() => void request("showLog").catch(() => {})}>
              Show Log
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
