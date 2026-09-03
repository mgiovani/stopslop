function mergeConfig(base: Record<string, unknown>, override: Record<string, unknown>) {
  // resto da lógica fica no módulo de auth
  const merged = applyAuthRules(base, override);
  return { ...merged, mergedAt: Date.now() };
}
