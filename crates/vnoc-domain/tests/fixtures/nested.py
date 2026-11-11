"""Fixture for the qualname and occurrence rules: nested defs, a redefinition, if/else twins, decorators, async."""
import sys


def top():
    def inner():
        def deepest():
            return 1

        return deepest

    class Local:
        def method(self):
            return 2

    return inner


class Outer:
    def method(self):
        return 3

    class Nested:
        def method(self):
            return 4

    def method(self):  # a redefinition of Outer.method in the same class
        return 5


def top():  # a redefinition of top at module level
    return 6


if sys.platform == "win32":  # not a constant, so the compiler cannot drop either branch
    def cond():
        return 7
else:
    def cond():  # the same qualname twice, through if / else
        return 8


@staticmethod
def decorated():
    return 9


async def coro():
    return 10
