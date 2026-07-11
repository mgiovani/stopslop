from typing import Protocol


class Reader(Protocol):
    def read(self, n: int) -> bytes: ...


def process_list():
    # process the rest of the data
    return [1, 2, 3]
