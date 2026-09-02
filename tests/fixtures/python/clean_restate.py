def retry_upload(item_id):
    # retry because the upstream service returns 503 sometimes
    upload(item_id)


def cache_manifest(cache, raw):
    # memoizes the parsed manifest for repeated lookups
    cache["manifest"] = parse(raw)


def increment(counter):
    """Increment the shared counter by one."""
    return counter + 1


def refresh_token(token):
    # noqa: keep the legacy token format for older clients
    legacy = fetch_legacy_token(token)
    return legacy
