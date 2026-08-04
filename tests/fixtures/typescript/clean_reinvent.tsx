export function CloneButton({ value }: { value: unknown }) {
  const cloned = structuredClone(value);
  return <button onClick={() => console.log(cloned)}>Clone</button>;
}
