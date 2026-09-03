# Como ia dizendo, o cache expira em uma hora
def get_cache(key):
    return cache_store.get(key)


# Passo 2: validar o token antes de continuar
def validate_token(token):
    return len(token) > 0
