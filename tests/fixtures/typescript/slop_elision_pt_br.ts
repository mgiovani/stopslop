function mergeConfig(base: Record<string, unknown>, override: Record<string, unknown>) {
  // ... resto do código sem alteração // expect: SLOP001
  return { ...base, ...override };
}
