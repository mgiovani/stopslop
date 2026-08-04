function cloneUser(user: Record<string, unknown>) {
  const clone = structuredClone(user);
  return clone;
}

function makeId() {
  return crypto.randomUUID();
}

function parseQuery(search: string) {
  return new URLSearchParams(search);
}

function padZero(value: string, width: number) {
  return value.padStart(width, '0');
}

function sleep(ms: number) {
  return new Promise((resolve) => resolve(undefined));
}
