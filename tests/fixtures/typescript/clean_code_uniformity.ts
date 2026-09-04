export type Ledger = Map<string, number>;

export type Entry = { account: string; amount: number };

export function apply(ledger: Ledger, entries: Entry[]): number {
  let total = 0;
  for (const { account, amount } of entries) {
    if (account.length === 0) {
      throw new Error("entry with no account");
    }
    if (!Number.isFinite(amount)) {
      throw new Error(`non-finite amount on ${account}`);
    }
    ledger.set(account, (ledger.get(account) ?? 0) + amount);
    total += amount;
  }
  return total;
}

export function balance(ledger: Ledger, account: string): number {
  return ledger.get(account) ?? 0;
}

// Largest-first, because a settlement run that stops early should still have
// cleared the balances that matter most.
export function settle(ledger: Ledger, budget: number): string[] {
  const owed = [...ledger.entries()]
    .filter(([, amount]) => amount > 0)
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));

  let remaining = budget;
  const settled: string[] = [];
  for (const [account, amount] of owed) {
    if (amount > remaining) {
      continue;
    }
    remaining -= amount;
    ledger.set(account, 0);
    settled.push(account);
  }
  return settled;
}

export function merge(left: Ledger, right: Ledger): Ledger {
  const out = new Map(left);
  for (const [account, amount] of right) {
    out.set(account, (out.get(account) ?? 0) + amount);
  }
  return out;
}

export function* nonzero(ledger: Ledger): Generator<[string, number]> {
  for (const pair of ledger) {
    if (pair[1] !== 0) {
      yield pair;
    }
  }
}

export function describe(ledger: Ledger): string {
  const parts: string[] = [];
  for (const [account, amount] of nonzero(ledger)) {
    parts.push(`${account}=${amount}`);
  }
  return parts.length === 0 ? "empty ledger" : parts.join(", ");
}

export function rebase(ledger: Ledger, floorValue: number): Ledger {
  const out: Ledger = new Map();
  for (const [account, amount] of ledger) {
    out.set(account, Math.max(amount, floorValue));
  }
  return out;
}

export function largest(ledger: Ledger): string | undefined {
  let best: string | undefined;
  let bestAmount = Number.NEGATIVE_INFINITY;
  for (const [account, amount] of ledger) {
    if (amount > bestAmount) {
      best = account;
      bestAmount = amount;
    }
  }
  return best;
}
