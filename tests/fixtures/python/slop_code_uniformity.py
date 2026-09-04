def normalize_account(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("account is empty")
    return value.replace(" ", "-")

def normalize_profile(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("profile is empty")
    return value.replace(" ", "-")

def normalize_session(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("session is empty")
    return value.replace(" ", "-")

def normalize_invoice(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("invoice is empty")
    return value.replace(" ", "-")

def normalize_payment(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("payment is empty")
    return value.replace(" ", "-")

def normalize_address(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("address is empty")
    return value.replace(" ", "-")

def normalize_contact(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("contact is empty")
    return value.replace(" ", "-")

def normalize_channel(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("channel is empty")
    return value.replace(" ", "-")

def normalize_message(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("message is empty")
    return value.replace(" ", "-")

def normalize_summary(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("summary is empty")
    return value.replace(" ", "-")

def normalize_receipt(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("receipt is empty")
    return value.replace(" ", "-")

def normalize_voucher(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("voucher is empty")
    return value.replace(" ", "-")

def normalize_booking(raw: str) -> str:
    value = raw.strip().lower()
    if len(value) == 0:
        raise ValueError("booking is empty")
    return value.replace(" ", "-")
# expect-line: 1 SLOP045
