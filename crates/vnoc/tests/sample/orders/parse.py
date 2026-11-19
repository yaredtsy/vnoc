"""Reading what the upstream system sends."""

import sys

if sys.version_info >= (3, 9):

    def normalize(text):
        return text.removeprefix("order:").strip()

else:

    def normalize(text):
        return text[len("order:") :].strip() if text.startswith("order:") else text.strip()


def split_items(text):
    return [part.strip() for part in text.split(",") if part.strip()]
