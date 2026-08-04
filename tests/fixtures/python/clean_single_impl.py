from abc import ABC, abstractmethod


class Storage(ABC):
    @abstractmethod
    def get(self, key):
        pass


class MemoryStorage(Storage):
    def get(self, key):
        return key


class RedisStorage(Storage):
    def get(self, key):
        return key
