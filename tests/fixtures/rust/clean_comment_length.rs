/// Pings the upstream health endpoint and reports whether it responded successfully this time,
/// retrying internally a small number of times before giving up and returning false so callers
/// never have to implement their own retry loop around this same flaky dependency themselves.
fn fetch_with_retry() -> bool {
    // Retries a few times before giving up; the upstream flakes briefly after each deploy.
    for _ in 0..3 {
        if upstream() {
            return true;
        }
    }
    false
}

fn upstream() -> bool {
    true
}
