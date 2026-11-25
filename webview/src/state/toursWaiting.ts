// What the host says while it asks the core again because the core said to (the first build is still running): the core's own words, to draw as a quiet line, or null when that
// is over. Nothing is made up here.

import { create } from "zustand";

import { on } from "../host.ts";

export const useToursWaiting = create<{ words: string | null }>(() => ({ words: null }));

on("toursWaiting", ({ words }) => useToursWaiting.setState({ words }));
