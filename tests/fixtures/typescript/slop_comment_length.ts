export function fetchWithRetry(): boolean {
  // This retry loop exists because the upstream service returns a transient failure during
  // its warm start, so the first call after a deploy almost always fails and we must not
  // surface that to the caller, otherwise every deploy would page the on-call engineer for
  // nothing and the dashboards would show a spike that is not real.
  for (let i = 0; i < 3; i++) {
    if (upstream()) {
      return true;
    }
  }
  return false;
}

function upstream(): boolean {
  return true;
}

// expect-line: 2 SLOP043
