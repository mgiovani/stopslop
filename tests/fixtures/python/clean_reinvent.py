import copy
from collections import defaultdict
from pathlib import Path


def sum_indexed(xs):
    total = 0
    for x in xs:
        total += x
    return total


def load_config(path):
    return Path(path).read_text()


def clone(obj):
    return copy.deepcopy(obj)


def bucket():
    groups = defaultdict(list)
    return groups
