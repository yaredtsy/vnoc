from pkg.calc import a
from pkg.db import Db, helper


class Service:
    def __init__(self):
        Db()

    def run(self):
        helper()

        def inner():
            return a()

        return inner()


class Plain:
    def go(self):
        return 1


class Twice:
    def __init__(self):
        helper()

    def __init__(self, x):
        a()
