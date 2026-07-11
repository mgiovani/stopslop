interface Config {
  endpoint: string;
}

function loadRaw(): unknown {
  return {};
}

const raw = loadRaw();

// standalone `as unknown` (not chained into a further cast) is a safe narrowing step
const w = raw as unknown;

const y = raw as const;

const obj: Record<string, string> = { a: "b" };
const data = obj as Record<string, string>;

const s = obj satisfies Record<string, string>;

function legacyFn(): number {
  return 1;
}

// @ts-expect-error – intentional, backwards-compatible shim
const result = legacyFn();

function someFunc(n: number): number {
  return n;
}

// @ts-ignore[2322] – assigning a narrower value is safe here
const val = someFunc(1);

/**
 * Use @ts-ignore if you must bypass type checking for legacy reasons.
 */
export function legacyAPI(x: unknown) {
  return x;
}
