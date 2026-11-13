import { Button } from "@/components/ui/button.tsx";
import { refresh } from "@/state/data.ts";

/** What the canvas says instead of cards: the first read of the code, or why it failed. */
export function CanvasMessage({ status, message }: { status: "loading" | "error"; message: string }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-sm">
      {status === "loading" ? (
        <p className="text-muted-foreground">Reading the code…</p>
      ) : (
        <>
          <p className="max-w-xl whitespace-pre-wrap text-destructive">{message}</p>
          <Button size="sm" onClick={() => refresh()}>
            Try Again
          </Button>
        </>
      )}
    </div>
  );
}
