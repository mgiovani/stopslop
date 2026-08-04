function getUser(id: string) {
  console.log('fetching', id);
  return fetchUser(id);
}

const getUserArrow = (id: string) => fetchUser(id, { cache: true });

function fetchUser(id: string, options?: { cache?: boolean }) {
  return { id, name: 'Alice' };
}
