from typing import Iterator

Ledger = dict[str, int]


def apply(ledger: Ledger, entries: list[tuple[str, int]]) -> int:
    total = 0
    for account, amount in entries:
        if not account:
            raise ValueError("entry with no account")
        ledger[account] = ledger.get(account, 0) + amount
        total += amount
    return total


def balance(ledger: Ledger, account: str) -> int:
    return ledger.get(account, 0)


# Largest-first, because a settlement run that stops early should still have
# cleared the balances that matter most.
def settle(ledger: Ledger, budget: int) -> list[str]:
    owed = sorted(
        ((account, amount) for account, amount in ledger.items() if amount > 0),
        key=lambda pair: (-pair[1], pair[0]),
    )

    remaining = budget
    settled = []
    for account, amount in owed:
        if amount > remaining:
            continue
        remaining -= amount
        ledger[account] = 0
        settled.append(account)
    return settled


def merge(left: Ledger, right: Ledger) -> Ledger:
    out = dict(left)
    for account, amount in right.items():
        out[account] = out.get(account, 0) + amount
    return out


def nonzero(ledger: Ledger) -> Iterator[tuple[str, int]]:
    return ((account, amount) for account, amount in ledger.items() if amount)


def describe(ledger: Ledger) -> str:
    parts = [f"{account}={amount}" for account, amount in nonzero(ledger)]
    if not parts:
        return "empty ledger"
    return ", ".join(parts)


def rebase(ledger: Ledger, floor_value: int) -> Ledger:
    out: Ledger = {}
    for account, amount in ledger.items():
        out[account] = amount if amount >= floor_value else floor_value
    return out


def largest(ledger: Ledger) -> str | None:
    best = None
    best_amount = float("-inf")
    for account, amount in ledger.items():
        if amount > best_amount:
            best = account
            best_amount = amount
    return best


def split_by_sign(ledger: Ledger) -> tuple[Ledger, Ledger]:
    credits: Ledger = {}
    debits: Ledger = {}
    for account, amount in ledger.items():
        target = credits if amount >= 0 else debits
        target[account] = amount
    return credits, debits


def totals(ledger: Ledger) -> tuple[int, int]:
    credits, debits = split_by_sign(ledger)
    return sum(credits.values()), sum(debits.values())
