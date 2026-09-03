class HitCounter:
    def __init__(self):
        self.contador = 0

    def record_hit(self):
        # incrementa o contador
        self.contador += 1

    def record_batch(self, n):
        self.contador += n  # incrementa o contador


def reset_state():
    # inicializa índice, número, função, variável, método e dicionário
    índice = número = função = variável = 0
    return índice, número, função, variável


# expect-line: 6 SLOP042
# expect-line: 10 SLOP042
# expect-line: 14 SLOP042
