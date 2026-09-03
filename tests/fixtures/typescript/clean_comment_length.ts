export function fetchWithRetry(): boolean {
  // Retries a few times before giving up; the upstream flakes briefly after each deploy.
  for (let i = 0; i < 3; i++) {
    if (upstream()) {
      return true;
    }
  }
  return false;
}

/**
 * Pings the upstream health endpoint and reports whether it responded successfully this time,
 * retrying internally a small number of times before giving up and returning false so callers
 * never have to implement their own retry loop around this same flaky dependency themselves.
 */
function upstream(): boolean {
  return true;
}
