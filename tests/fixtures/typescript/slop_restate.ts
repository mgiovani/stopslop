class HitCounter {
  count = 0;

  recordHit(): void {
    // increment the count
    this.count += 1;
  }

  recordBatch(n: number): void {
    this.count += n; // increment the count
  }
}

// expect-line: 5 SLOP042
// expect-line: 10 SLOP042
