import os


def process_email(email: str) -> bool:
    """Validate email.

    Args:
        email: user email, e.g. user@example.com

    Returns:
        True if valid.
    """
    return "@" in email


API_URL = os.environ.get("API_URL", "https://api.production.com")
