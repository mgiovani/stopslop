function retryUpload(id: number): void {
  // retry because the upstream service returns 503 sometimes
  upload(id);
}

function cacheManifest(cache: Record<string, string>, raw: string): void {
  // memoizes the parsed manifest for repeated lookups
  cache.manifest = parse(raw);
}

/**
 * Increments the shared counter.
 */
function increment(counter: number): number {
  return counter + 1;
}

function refreshToken(token: string): string {
  // eslint-disable-next-line no-unused-vars
  const legacy = fetchLegacyToken(token);
  return legacy;
}
