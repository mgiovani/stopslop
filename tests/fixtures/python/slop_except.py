import json


def load_bare():
    try:
        return json.load(open("data.json"))
    except:  # expect: SLOP006
        pass


def load_bare_log():
    try:
        return json.load(open("data.json"))
    except:  # expect: SLOP006
        print("failed to load")


def load_broad():
    try:
        return json.load(open("data.json"))
    except Exception:  # expect: SLOP006
        pass


def load_broad_named_log():
    try:
        return json.load(open("data.json"))
    except BaseException as e:  # expect: SLOP006
        log(e)
