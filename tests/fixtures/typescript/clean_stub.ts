interface Reader {
  read(size: number): string;
}

abstract class Base {
  abstract process(): void;
}

class RealImpl {
  process(): void {
    console.log("processed");
  }
}
