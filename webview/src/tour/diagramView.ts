// A diagram opened large (docs/26-tours/05-on-screen.md §5, G12): it is fitted to the overlay whole, with its aspect, and the reader zooms and pans to read it closer. Pure: the
// overlay measures its room and applies what these give.

/** Where the diagram is in the overlay: its scale, and the place of its top left corner in the overlay's own pixels. */
export interface View {
  scale: number;
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

/** The most a diagram is magnified, whatever its size (a small one opens at most twice its natural size). */
export const MAX_SCALE = 8;
/** How much of the diagram must stay in the overlay when it is panned away (px). */
export const KEEP = 48;
/** A step of `+` and `−`, and of the wheel per 100 px of `deltaY`. */
export const STEP = 1.25;

/** The natural size of an SVG Mermaid made: the third and fourth numbers of its `viewBox`. null when it has none. */
export function naturalSize(svg: string): Size | null {
  const box = /<svg\b[^>]*?\sviewBox="([^"]*)"/i.exec(svg)?.[1];
  const parts = box?.trim().split(/[\s,]+/).map(Number);
  return parts && parts.length === 4 && parts.every(Number.isFinite) && parts[2] > 0 && parts[3] > 0 ? { width: parts[2], height: parts[3] } : null;
}

/** The scale that shows the whole diagram in `room`, aspect kept: scaled down when it is bigger, up to twice its size when it is smaller. */
export function fitScale(natural: Size, room: Size): number {
  if (!(room.width > 0 && room.height > 0)) return 1;
  return Math.min(2, room.width / natural.width, room.height / natural.height);
}

/** The diagram whole and centred in `room`. */
export function fitView(natural: Size, room: Size): View {
  const scale = fitScale(natural, room);
  return { scale, x: (room.width - natural.width * scale) / 2, y: (room.height - natural.height * scale) / 2 };
}

/** Keeps at least [`KEEP`] px of the diagram in the room (or all of it, when it is smaller than that), so it can never be panned out of reach. */
export function kept(view: View, natural: Size, room: Size): View {
  const w = natural.width * view.scale;
  const h = natural.height * view.scale;
  const keepX = Math.min(KEEP, w);
  const keepY = Math.min(KEEP, h);
  return {
    scale: view.scale,
    x: Math.min(room.width - keepX, Math.max(keepX - w, view.x)),
    y: Math.min(room.height - keepY, Math.max(keepY - h, view.y)),
  };
}

/** Zooms by `factor` about the point `at` (in the room's pixels): the part of the diagram under it stays under it. The scale stays between half the fit and [`MAX_SCALE`]. */
export function zoomAt(view: View, factor: number, at: { x: number; y: number }, natural: Size, room: Size): View {
  const low = fitScale(natural, room) / 2;
  const scale = Math.min(MAX_SCALE, Math.max(low, view.scale * factor));
  const k = scale / view.scale;
  return kept({ scale, x: at.x - (at.x - view.x) * k, y: at.y - (at.y - view.y) * k }, natural, room);
}

/** Moves the diagram by `dx` and `dy` px. */
export const panBy = (view: View, dx: number, dy: number, natural: Size, room: Size): View => kept({ ...view, x: view.x + dx, y: view.y + dy }, natural, room);

/** The factor of a wheel turn of `deltaY` px (three lines of a mouse wheel are about 100): `STEP` for each hundred, and the reverse for the other way. */
export const wheelFactor = (deltaY: number): number => Math.pow(STEP, -deltaY / 100);
