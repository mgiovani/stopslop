function processPayment(): void {
  throw new Error("Not implemented");
}

class PaymentService {
  charge(): void {
    throw new Error("Not implemented");
  }
}

const refund = (): void => {
  throw new Error("Not implemented");
};

// expect-line: 1 SLOP008
// expect-line: 6 SLOP008
// expect-line: 11 SLOP008
