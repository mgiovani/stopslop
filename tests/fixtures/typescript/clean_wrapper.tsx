function Wrapper(id: string) {
  console.log('rendering', id);
  return Inner(id);
}

function Inner(id: string) {
  return <div>{id}</div>;
}
