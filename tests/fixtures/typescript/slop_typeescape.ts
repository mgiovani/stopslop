interface Config {
  endpoint: string;
}

function loadRaw(): unknown {
  return {};
}

const data = loadRaw();
const x = data as any; // expect: SLOP007

const raw = loadRaw();
const config = raw as unknown as Config; // expect: SLOP007

function unsafeOp(): number {
  return 1;
}

// @ts-ignore expect: SLOP007
const result = unsafeOp();

function someFunc(n: number): number {
  return n;
}

// @ts-nocheck expect: SLOP007
const cfg = someFunc(1);
