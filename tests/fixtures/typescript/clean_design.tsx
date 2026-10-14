export function Hero({ tone }: { tone: string }) {
  return (
    <section>
      <h1 className="text-slate-900 from-violet-500">Harbor</h1>
      <h2 style={{ backgroundImage: "linear-gradient(90deg, #ff7e5f, #feb47b)" }}>Tools</h2>
      <button className={`px-4 ${tone}`}>Start</button>
      <a className="bg-purple-50 text-violet-900 to-violet-500">Docs</a>
    </section>
  );
}
