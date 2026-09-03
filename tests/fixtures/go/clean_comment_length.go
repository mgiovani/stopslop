package main

// FetchWithRetry pings the upstream health endpoint a few times before giving up, retrying
// internally so callers never have to implement their own retry loop around this same flaky
// dependency that intermittently fails for a moment right after each fresh deploy completes.
func FetchWithRetry() bool {
	// Retries a few times before giving up; the upstream flakes briefly after each deploy.
	for i := 0; i < 3; i++ {
		if upstream() {
			return true
		}
	}
	return false
}

func upstream() bool {
	return true
}
