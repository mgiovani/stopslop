from abc import ABC, abstractmethod
from typing import Protocol, overload


class Processor(ABC):
    @abstractmethod
    def process(self, data):
        pass


class Reader(Protocol):
    def read(self, n: int) -> bytes:
        ...


@overload
def convert(val: int) -> str: ...


@overload
def convert(val: str) -> int: ...


def convert(val):
    return str(val) if isinstance(val, int) else int(val)


def documented(data):
    """Process real data."""
    return data.strip()
