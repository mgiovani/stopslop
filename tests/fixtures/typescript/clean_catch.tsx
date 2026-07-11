function risky(): string {
  return "ok";
}

function RethrowCatch() {
  try {
    risky();
  } catch (e) {
    throw e;
  }
  return null;
}

function RecoveryCatch() {
  let result: string | null = null;
  try {
    result = risky();
  } catch (e) {
    result = "fallback";
  }
  return result;
}
