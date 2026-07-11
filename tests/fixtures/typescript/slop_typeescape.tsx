interface Props {
  label: string;
}

function loadRaw(): unknown {
  return {};
}

const data = loadRaw();
const p = data as any; // expect: SLOP007

const raw = loadRaw();
const props = raw as unknown as Props; // expect: SLOP007

// @ts-ignore expect: SLOP007
export function Widget() {
  return <div>{props.label}</div>;
}
