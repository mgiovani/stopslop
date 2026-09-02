struct Cache {
    manifest: String,
}

fn retry_upload(id: u32) {
    // retry because the upstream service returns 503 sometimes
    upload(id);
}

fn cache_manifest(cache: &mut Cache, raw: &str) {
    // memoizes the parsed manifest for repeated lookups
    cache.manifest = parse(raw);
}

/// Increments the shared counter.
fn increment(counter: &mut i32) {
    *counter += 1;
}

fn refresh_token(token: &mut String) {
    // clippy allow dead code for the reserved token field
    *token = fetch_token();
}
