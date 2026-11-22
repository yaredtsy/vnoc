# Adds two numbers.
def add(a: Nat, b: Nat) -> Nat:
  match a:
    case Zero{}: b
    case Succ{p}: Succ{add(p, b)}

@unsafe def raw(x: U32) -> U32: x

def twice(~f: U32 -> U32, x: U32) -> U32: f(f(x))

# the claim
law add_zero:
  for n: Nat
  Eq(Nat, add(n, 0n), n)

# the proof
def add_zero(n):
  match n:
    case Zero{}: {==}
    case Succ{p}: {==}

# A list.
type List<a, -A: Kind(a)> is Kind(a):
  Nil{}
  Con{head: A, tail: List<a, A>}

def Nat.double(n: Nat) -> Nat: add(n, n)
