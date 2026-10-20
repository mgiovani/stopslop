export function Footer({ year }: { year: number }) {
  return (
    <footer>
      <p>Home · Docs · Blog</p> {/* expect: SLOP057 */}
      <small>{`v${year} • stable`}</small> {/* expect: SLOP057 */}
    </footer>
  );
}
