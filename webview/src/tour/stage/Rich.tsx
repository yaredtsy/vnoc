import { pieces } from "../text.ts";

/** A model's words, drawn as text with `code` and *emphasis* (`../text.ts`): React makes the elements, so nothing in them is ever HTML. */
export function Rich({ text }: { text: string }) {
  return (
    <>
      {pieces(text).map((p, i) => (p.kind === "code" ? <code key={i}>{p.text}</code> : p.kind === "em" ? <em key={i}>{p.text}</em> : p.text))}
    </>
  );
}
