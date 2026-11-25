// The renderer of a step's `title` and `text` (docs/26-tours/05-on-screen.md §7). They are a model's output, or came by `git pull`: this knows two things, a
// `code` span and *emphasis*, and everything else is text. It returns pieces for React to draw as text; it never makes HTML, so a `<b>` is the characters
// `<b>`. It is not the docs panel's Markdown pipeline.

export type Piece = { kind: "text" | "code" | "em"; text: string };

// a code span: backticks around something that is not empty and has no backtick in it. Emphasis: one `*` not touching another, around text that does not start or
// end with a space and has no `*`; `**x**` and a lone `*` (`a * b`) are text.
const PIECE = /`([^`\n]+)`|(?<!\*)\*(?!\*)(?!\s)([^*\n]+?)(?<!\s)\*(?!\*)/g;

export function pieces(text: string): Piece[] {
  const out: Piece[] = [];
  let from = 0;
  const push = (kind: Piece["kind"], s: string) => {
    if (s === "") return;
    const last = out[out.length - 1];
    if (last && last.kind === kind && kind === "text") last.text += s;
    else out.push({ kind, text: s });
  };
  for (const m of text.matchAll(PIECE)) {
    push("text", text.slice(from, m.index));
    if (m[1] !== undefined) push("code", m[1]);
    else push("em", m[2]);
    from = m.index + m[0].length;
  }
  push("text", text.slice(from));
  return out;
}
