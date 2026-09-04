export function normalizeAccount(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("account is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeProfile(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("profile is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeSession(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("session is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeInvoice(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("invoice is empty");
  }
  return value.split(" ").join("-");
}

export function normalizePayment(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("payment is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeAddress(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("address is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeContact(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("contact is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeChannel(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("channel is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeMessage(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("message is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeSummary(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("summary is empty");
  }
  return value.split(" ").join("-");
}

export function normalizeReceipt(raw: string): string {
  const value = raw.trim().toLowerCase();
  if (value.length === 0) {
    throw new Error("receipt is empty");
  }
  return value.split(" ").join("-");
}
// expect-line: 1 SLOP045
