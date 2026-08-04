from abc import ABC, abstractmethod


class Storage(ABC):  # expect: SLOP040
    @abstractmethod
    def get(self, key):
        pass


class MemoryStorage(Storage):
    def get(self, key):
        return key
