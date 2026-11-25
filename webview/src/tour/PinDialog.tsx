import { useEffect, useRef } from "react";

import { on, post, request } from "../host.ts";
import { closePinDialog, commitAndPin, openPinDialog, setAlways, setMessage, usePinDialog } from "./pinDialog.ts";

/**
 * **Commit and Pin…** (docs/26-tours/05-on-screen.md §3): the one dialog a tour has, over the canvas page. The files that will be committed, each a link to its diff against HEAD; the message,
 * editable; *Always do this in this project*; **Commit and Pin** and **Cancel**. Cancel commits nothing. It is opened by the cover's button, and by the host (the row's menu, the **Pin**
 * message), which has the page open it with `tourDialog`, or leaves the tour for the page to take when it starts.
 */
export function PinDialog() {
  const dialog = usePinDialog((s) => s.dialog);
  const box = useRef<HTMLTextAreaElement>(null);
  // the host chose **Commit and Pin…** for a tour: now, if the page is here, or at start if it was not
  useEffect(() => {
    const take = () => void request("tourDialogTake").then((got) => got && void openPinDialog(got.id, got.title)).catch(() => {});
    take();
    return on("tourDialog", ({ id, title }) => {
      void openPinDialog(id, title);
      take(); // it was taken: the host holds none now
    });
  }, []);
  // what the suite reads of the dialog
  useEffect(() => {
    post({ type: "pinDialog", dialog: dialog ? { tour: dialog.tour, files: dialog.files ?? [], message: dialog.message, always: dialog.always, error: dialog.error } : null });
  }, [dialog?.tour, dialog?.files?.join("\n"), dialog?.message, dialog?.always, dialog?.error]); // eslint-disable-line react-hooks/exhaustive-deps
  // the suite cannot click in a webview: the same buttons, by the host's `viewCommand`
  useEffect(
    () =>
      on("viewCommand", ({ name, key, text }) => {
        if (name !== "pinDialog" || !key) return;
        const d = usePinDialog.getState().dialog;
        if (key === "commit") void commitAndPin();
        else if (key === "cancel") closePinDialog();
        else if (key === "always") setAlways(!(d?.always ?? false));
        else if (key === "message") setMessage(text ?? "");
        else if (key.startsWith("file:")) void request("openChange", { path: key.slice(5) }).catch(() => {});
      }),
    [],
  );
  const open = dialog !== null;
  useEffect(() => {
    if (open) box.current?.focus();
  }, [open]);
  if (!dialog) return null;
  const files = dialog.files;
  return (
    <div className="pin-backdrop" role="presentation" onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), closePinDialog())}>
      <div role="dialog" aria-modal="true" aria-labelledby="pin-title" className="pin-dialog" data-part="pin-dialog">
        <h3 id="pin-title" className="pin-title">
          Commit and Pin
        </h3>
        {files === null ? (
          <p className="pin-dim">Looking at what is not committed…</p>
        ) : files.length > 0 ? (
          <>
            <p className="pin-dim">These files will be committed, and nothing else:</p>
            <ul className="pin-files" data-part="pin-files">
              {files.map((f) => (
                <li key={f}>
                  <button type="button" className="pin-file" title="Show the changes in this file" onClick={() => void request("openChange", { path: f }).catch(() => {})}>
                    {f}
                  </button>
                </li>
              ))}
            </ul>
          </>
        ) : null}
        <label className="pin-label">
          Message
          <textarea ref={box} rows={2} className="pin-message" value={dialog.message} disabled={dialog.busy} onChange={(e) => setMessage(e.target.value)} />
        </label>
        <label className="pin-check">
          <input type="checkbox" checked={dialog.always} disabled={dialog.busy} onChange={(e) => setAlways(e.target.checked)} />
          Always do this in this project
        </label>
        {dialog.error && (
          <p className="pin-error" role="alert" data-part="pin-error">
            {dialog.error}
          </p>
        )}
        <div className="stage-buttons">
          <button type="button" className="stage-btn stage-primary" disabled={dialog.busy || files === null || files.length === 0} onClick={() => void commitAndPin()}>
            Commit and Pin
          </button>
          <button type="button" className="stage-btn stage-secondary" onClick={closePinDialog}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
