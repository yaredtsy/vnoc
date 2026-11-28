/** How many times longer the window suite waits than on a developer's machine: what `VNOC_TEST_SLOWDOWN` says, else 3 on CI (`CI` is set) and 1 here. The one formula: the host's ticket limit and the suites' deadlines both use it. */
export function slowdown(env: { VNOC_TEST_SLOWDOWN?: string; CI?: string }): number {
  return Number(env.VNOC_TEST_SLOWDOWN) || (env.CI ? 3 : 1);
}
