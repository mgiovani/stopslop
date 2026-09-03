class HitCounter {
  contador = 0;

  registrarHit(): void {
    // incrementa o contador
    this.contador += 1;
  }

  registrarLote(n: number): void {
    this.contador += n; // incrementa o contador
  }
}

// expect-line: 5 SLOP042
// expect-line: 10 SLOP042
