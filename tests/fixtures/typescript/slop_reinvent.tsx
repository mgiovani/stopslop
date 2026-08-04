export function CloneButton({ value }: { value: unknown }) {
  const cloned = JSON.parse(JSON.stringify(value)); // expect: SLOP037
  return <button onClick={() => console.log(cloned)}>Clone</button>;
}
