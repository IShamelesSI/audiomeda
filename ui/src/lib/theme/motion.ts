// Svelte transitions take numbers, so read the motion tokens from CSS at runtime.
// That keeps tokens.css the single place timings are set (including reduced motion).

export function ms(token: "--dur-fast" | "--dur" | "--dur-slow"): number {
  const v = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  return parseFloat(v) || 0;
}
