def fetch_with_retry():
    # This retry loop exists because the upstream service returns a transient failure during
    # its warm start, so the first call after a deploy almost always fails and we must not
    # surface that to the caller, otherwise every deploy would page the on-call engineer for
    # nothing and the dashboards would show a spike that is not real.
    for _ in range(3):
        if upstream():
            return True
    return False


def upstream():
    return True


# expect-line: 2 SLOP043
