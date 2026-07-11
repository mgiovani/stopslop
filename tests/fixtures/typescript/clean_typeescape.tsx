interface Props {
  label: string;
}

function loadRaw(): unknown {
  return {};
}

const raw = loadRaw();

// standalone `as unknown` (not chained into a further cast) is a safe narrowing step
const w = raw as unknown;

const props: Props = { label: (raw as const) === undefined ? "x" : "x" };

export function Widget() {
  return <div>{props.label}</div>;
}
