import ./lib.bend as Lib

def helper(x: U32) -> U32:
  x

# ID: 0MAIN000000000000000000TWO
def twice(x: U32) -> U32:
  helper(helper(x))

def uses_lib(x: U32) -> U32:
  helper(Lib.inc(x))

def Nat.double(n: U32) -> U32:
  helper(n)

def forever(n: U32) -> U32:
  forever(n)

def a(n: U32) -> U32:
  b(n)

def b(n: U32) -> U32:
  c(n)

def c(n: U32) -> U32:
  helper(n)

def top(n: U32) -> U32:
  a(n)
