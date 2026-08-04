def sum_indexed(xs):
    total = 0
    for i in range(len(xs)):  # expect: SLOP037
        total += xs[i]
    return total


def load_config(path):
    return open(path).read()  # expect: SLOP037


def deep_copy(obj):  # expect: SLOP037
    return obj


def bucket(groups, key):
    if key not in groups:  # expect: SLOP037
        groups[key] = []
    groups[key].append(key)


import re

EMAIL_RE = re.compile(r"[^\s@]+@[^\s@]+")  # expect: SLOP037
