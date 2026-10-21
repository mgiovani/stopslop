export function Footer({ year }: { year: number }) {
  return (
    <footer>
      <p>Home · Docs</p>
      <small>{`v${year}`}</small>
      <a href="/dalle">DALL·E</a>
    </footer>
  );
}
