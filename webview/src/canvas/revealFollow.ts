// A reveal frames one card (`CenterOn`), and the pane may change size a moment later: a pick from the search opens the docs tab beside the canvas, and that tab takes half of the canvas's editor group. A fit that was worked
// out for the old width is then centred on the wrong place. Pure: the window in which the pane's change is followed by framing the same card again.

/** How long after a reveal's fit a change of the pane's size frames the card again. The docs tab is up within about 0.3 s of a click; the window is not so long that a person's own resize is taken over. */
export const FOLLOW_MS = 1500;

/** `fitAt`: when the last reveal framed its card (null: none did); `now`: the same clock. */
export const followsReveal = (fitAt: number | null, now: number): boolean => fitAt !== null && now - fitAt >= 0 && now - fitAt <= FOLLOW_MS;
