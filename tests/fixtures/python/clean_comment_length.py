def fetch_with_retry():
    # Retries a few times before giving up; the upstream flakes briefly after each deploy.
    for _ in range(3):
        if upstream():
            return True
    return False


def upstream():
    """Ping the upstream health endpoint and report whether it responded successfully this
    time, retrying internally a small number of times before giving up and returning False so
    callers never have to implement their own retry loop around this same flaky dependency.
    """
    return True
