import os


def cleanup():
    try:
        os.remove("/tmp/scratch")
    except:
        pass
