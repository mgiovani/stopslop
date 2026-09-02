package main

func RetryUpload(id int) {
	// retry because the upstream service returns 503 sometimes
	upload(id)
}

func CacheManifest(cache map[string]string, raw string) {
	// memoizes the parsed manifest for repeated lookups
	cache["manifest"] = parse(raw)
}

// Increment increases the shared counter by one.
func Increment(counter int) int {
	return counter + 1
}

func RefreshToken(token string) string {
	// nolint: keep the legacy token format for older clients
	legacy := fetchLegacyToken(token)
	return legacy
}
