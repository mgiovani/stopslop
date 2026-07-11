def risky():
    return 1


def handle(data):
    return data


def parse_int(val):
    try:
        return int(val)
    except ValueError:
        return 0


def compute():
    try:
        return risky()
    except Exception:
        raise


def process(data):
    try:
        return handle(data)
    except Exception:
        return None
