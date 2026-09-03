// Como ia dizendo, o cache expira em uma hora
export function getCache(key: string) {
  return cacheStore.get(key);
}

// Passo 2: validar o token antes de continuar
export function validateToken(token: string) {
  return token.length > 0;
}
