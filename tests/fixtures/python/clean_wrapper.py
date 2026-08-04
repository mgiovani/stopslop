def get_user(user_id):
    print("fetching", user_id)
    return fetch_user(user_id)


def fetch_user(user_id):
    return {"id": user_id, "name": "Alice"}
