import ./lib.bend as Lib

def helper(x: U32) -> U32:
  x

def uses_lib(x: U32) -> U32:
  helper(Lib.inc(x))
