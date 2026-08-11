/**
 * Canonical narrativegoldmine page URLs for graph nodes.
 *
 * The site is a path-routed SPA (`/page/:pageName`, BrowserRouter with the
 * gh-pages 404 redirect trick) whose PageView resolves `:pageName` against
 * the slug-keyed `api/pages/<slug>.json`. Slugs are authored per page
 * (vc:slug in the corpus JSON-LD) and are NOT always derivable from the
 * label ("3D Asset" → "3-d-asset", "2D LiDAR" → "2-d-li-dar"), so prefer
 * the server-provided identifiers: a node's `metadataId` IS the slug, and
 * `page_iri`/`class_iri` (`urn:visionflow:page:<slug>`) end with it. The
 * slugified label is a last-resort fallback that matches the pipeline's
 * slugify for simple titles. The legacy `#/page/<Title>` hash route
 * predates the path router and no longer resolves.
 */

const PAGE_BASE = 'https://narrativegoldmine.com/page/';

/** Pipeline-compatible slugify: `re.sub(r'[^a-z0-9]+', '-', s.lower()).strip('-')` */
export function slugifyLabel(label: string): string {
  return label
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

interface PageLinkSource {
  label?: string;
  metadataId?: string;
  metadata_id?: string;
  metadata?: Record<string, unknown> | null;
}

/** Resolve a node to its narrativegoldmine page URL, or null when nothing
 *  identifies the page. Explicit metadata URLs always win. */
export function nodePageUrl(node: PageLinkSource): string | null {
  const meta = (node.metadata ?? {}) as Record<string, unknown>;

  const explicit = (meta.page_url ?? meta.pageUrl ?? meta.url) as string | undefined;
  if (explicit) return explicit;

  const iri = (meta.page_iri ?? meta.class_iri) as string | undefined;
  const identifier =
    node.metadataId ??
    node.metadata_id ??
    (meta.slug as string | undefined) ??
    (iri && iri.includes(':') ? iri.slice(iri.lastIndexOf(':') + 1) : undefined) ??
    node.label;

  // Some populations carry a title-shaped metadataId ("Blockchain" from the
  // file stem) rather than the kebab slug. slugifyLabel is idempotent on
  // real slugs ("modular-blockchain" → itself), so normalising every
  // identifier is safe and repairs the title-shaped ones.
  const slug = identifier ? slugifyLabel(identifier) : undefined;
  return slug ? `${PAGE_BASE}${encodeURIComponent(slug)}` : null;
}
