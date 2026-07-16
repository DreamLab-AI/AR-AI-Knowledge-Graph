/**
 * heatColor — turn normalised attention heat (0..1) into a per-instance colour
 * brightening for the WebGL gem/orb populations.
 *
 * The metadata-texture / aGlowMeta path folds heat into the emissive `_rB` term,
 * but on WebGL that additive glow is swamped by the shared base emissive, so a
 * touched gem reads no brighter than an idle one. The visible, provably per-
 * instance signal on WebGL is `instanceColor`: it multiplies the built-in
 * MeshStandardMaterial diffuse (`<color_fragment>`), so scaling it up literally
 * brightens the node in place.
 *
 * Heat must BRIGHTEN, never recolour: a hot community-red gem becomes a brighter
 * red, not white. A naive `rgb *= (1 + heat*k)` with a hard clamp desaturates
 * toward white the moment any channel saturates. Instead we cap the gain so the
 * brightest channel reaches at most 1.0 — the R:G:B ratio (hue AND saturation)
 * is preserved exactly, only luminance rises. This is the HSV "raise V, hold H/S"
 * brighten expressed as a single scalar multiply the caller can apply zero-alloc.
 */

/** Luminance gain per unit heat. A fresh touch (heat ~0.49) lifts up to ~1.39x,
 *  a hammered node (heat ~0.86) up to ~1.69x — a clear pop, capped per colour so
 *  it never clips to white. */
export const HEAT_BRIGHTEN_K = 0.8;

/** Uncapped multiplicative gain for a normalised 0..1 heat. Exported for tests. */
export function heatGain(heat: number): number {
  return 1 + Math.max(0, heat) * HEAT_BRIGHTEN_K;
}

/**
 * Ratio-preserving brighten factor for a base RGB (0..1) at the given heat.
 * Returns a scalar `f >= 1` such that `(r,g,b) * f` brightens the colour while
 * keeping the brightest channel <= 1, so hue and saturation survive (never white)
 * and heat never darkens. `heat = 0` (or an already-saturated colour) is identity.
 */
export function heatBrightenFactor(r: number, g: number, b: number, heat: number): number {
  const gain = heatGain(heat);
  const maxc = Math.max(r, g, b);
  if (maxc <= 0) return gain; // black stays black; nothing to over-drive
  // Cap so max channel hits at most 1 (ratio preserved), and floor at 1 so an
  // already-clipping base colour is left untouched rather than dimmed.
  return Math.max(1, Math.min(gain, 1 / maxc));
}
