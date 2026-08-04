interface Storage { // expect: SLOP040
  get(key: string): string;
}

class MemoryStorage implements Storage {
  get(key: string): string {
    return key;
  }
}

abstract class Handler { // expect: SLOP040
  abstract handle(): void;
}

class DefaultHandler extends Handler {
  handle(): void {}
}
