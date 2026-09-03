class HitCounter:
    def __init__(self):
        self.contador = 0

    def record_hit(self):
        # incrementa o contador
        self.contador += 1

    def record_batch(self, n):
        self.contador += n  # incrementa o contador


# expect-line: 6 SLOP042
# expect-line: 10 SLOP042
