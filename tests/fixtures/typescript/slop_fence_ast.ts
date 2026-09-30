```ts
export function f(y: unknown) {
  const x = y as any; // expect: SLOP007
  return x;
}
```
// expect-line: 1 SLOP003
// expect-line: 6 SLOP003
