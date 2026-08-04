def get_user(user_id):  # expect: SLOP039
    return fetch_user(user_id)


def fetch_user(user_id):
    return {"id": user_id, "name": "Alice"}
