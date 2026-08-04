interface Storage {
  get(key: string): string;
}

class MemoryStorage implements Storage {
  get(key: string): string {
    return key;
  }
}

class RedisStorage implements Storage {
  get(key: string): string {
    return key;
  }
}
