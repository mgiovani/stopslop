use std::collections::BTreeMap;

pub type Ledger = BTreeMap<String, i64>;

pub fn apply(ledger: &mut Ledger, entries: &[(String, i64)]) -> Result<i64, String> {
    let mut total = 0;
    for (account, amount) in entries {
        if account.is_empty() {
            return Err("entry with no account".to_string());
        }
        let slot = ledger.entry(account.clone()).or_insert(0);
        match slot.checked_add(*amount) {
            Some(next) => {
                *slot = next;
                total += *amount;
            }
            None => return Err(format!("overflow on {account}")),
        }
    }
    Ok(total)
}

pub fn balance(ledger: &Ledger, account: &str) -> i64 {
    ledger.get(account).copied().unwrap_or_default()
}

/// Accounts are settled largest-first because a partial settlement run that stops early should
/// still have cleared the balances that matter most.
pub fn settle(ledger: &mut Ledger, budget: i64) -> Vec<String> {
    let mut owed: Vec<(String, i64)> = ledger
        .iter()
        .filter(|(_, &v)| v > 0)
        .map(|(k, &v)| (k.clone(), v))
        .collect();
    owed.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut remaining = budget;
    let mut settled = Vec::new();
    for (account, amount) in owed {
        if amount > remaining {
            continue;
        }
        remaining -= amount;
        ledger.insert(account.clone(), 0);
        settled.push(account);
    }
    settled
}

pub fn merge(left: Ledger, right: Ledger) -> Ledger {
    let mut out = left;
    for (account, amount) in right {
        *out.entry(account).or_insert(0) += amount;
    }
    out
}

pub fn nonzero(ledger: &Ledger) -> impl Iterator<Item = (&String, &i64)> {
    ledger.iter().filter(|(_, &v)| v != 0)
}

pub fn describe(ledger: &Ledger) -> String {
    let mut parts = Vec::new();
    for (account, amount) in nonzero(ledger) {
        parts.push(format!("{account}={amount}"));
    }
    if parts.is_empty() {
        return "empty ledger".to_string();
    }
    parts.join(", ")
}
