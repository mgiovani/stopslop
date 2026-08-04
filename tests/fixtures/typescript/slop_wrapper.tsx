function Wrapper(id: string) { // expect: SLOP039
  return Inner(id);
}

function Inner(id: string) {
  return <div>{id}</div>;
}
