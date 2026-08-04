function getUser(id: string) { // expect: SLOP039
  return fetchUser(id);
}

const getUserArrow = (id: string) => fetchUser(id); // expect: SLOP039

function fetchUser(id: string) {
  return { id, name: 'Alice' };
}
