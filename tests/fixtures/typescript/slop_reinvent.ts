function cloneUser(user: Record<string, unknown>) {
  return JSON.parse(JSON.stringify(user)); // expect: SLOP037
}

function makeId() {
  return Math.random().toString(36); // expect: SLOP037
}

function parseQuery(search: string) {
  const parts = search.split('&'); // expect: SLOP037
  return parts.map((p) => p.split('='));
}

function padZero(value: string, width: number) {
  let str = value;
  while (str.length < width) { // expect: SLOP037
    str = '0' + str;
  }
  return str;
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms)); // expect: SLOP037
}

const EMAIL_RE = /^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$/; // expect: SLOP037
