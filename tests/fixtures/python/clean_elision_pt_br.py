def merge_config(base, override):
    # resto da lógica fica no módulo de auth
    merged = apply_auth_rules(base, override)
    return {**merged, "merged_at": time.time()}
