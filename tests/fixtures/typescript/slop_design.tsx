export function Hero() {
  return (
    <section>
      <h1 className="bg-clip-text bg-gradient-to-r from-violet-500 to-indigo-500">Harbor</h1> {/* expect: SLOP052 SLOP053 */}
      <h2 style={{ backgroundImage: "linear-gradient(135deg, #667eea, #764ba2)", WebkitBackgroundClip: "text" }}>Tools</h2> {/* expect: SLOP052 SLOP053 */}
      <button className="bg-[#8b5cf6] px-4">Start</button> {/* expect: SLOP054 */}
    </section>
  );
}
