def b():
    return 1


def c():
    return 2


def a():
    b()
    c()
    b()


def fact(n):
    if n:
        fact(n - 1)
    return 1


def accents():
    s = "é😀"; b()
