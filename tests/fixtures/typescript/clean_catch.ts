function risky(): string {
  return "ok";
}
function fallback(): string {
  return "fallback";
}
function optional(): void {}
function reportError(_e: unknown): void {}

function rethrowCatch() {
  try {
    risky();
  } catch (e) {
    throw e;
  }
}

function returnCatch() {
  try {
    return risky();
  } catch (e) {
    return null;
  }
}

function recoveryAssignCatch() {
  let result;
  try {
    result = risky();
  } catch (e) {
    result = fallback();
  }
  return result;
}

function intentCatch() {
  try {
    optional();
  } catch (e) {
    // Intentional: operation is optional, ignore failure
  }
}

function nonConsoleCallCatch() {
  try {
    risky();
  } catch (e) {
    reportError(e);
  }
}
