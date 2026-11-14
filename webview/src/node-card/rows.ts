// The widths that line the rows of a card up (docs/16-ui-redesign/04-node-cards.md §1): the header's chevron, kind icon and gaps decide where the
// name starts, and the meta line and the footer start there too, so the three rows read as one column. One place, so a change of an icon moves all three.

/** the header's left padding, the chevron's box, the kind icon's box, and the gap between each (Tailwind `gap-1`) */
export const HEADER_PAD = 4;
export const CHEVRON = 18;
export const KIND_ICON = 14;
export const GAP = 4;

/** where the name's first letter is, from the left edge of the card's content */
export const NAME_X = HEADER_PAD + CHEVRON + GAP + KIND_ICON + GAP;

/** a footer chip has 6 px of padding of its own (`px-1.5`): the footer starts that much before, so the chip's TEXT is under the name */
export const CHIP_PAD = 6;
