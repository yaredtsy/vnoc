# ID: 3RGJ87CMS6TK775XRJV8BW977Y
def add(a: Nat, b: Nat) -> Nat:
  match a:
    case Zero{}: b
    case Succ{p}: Succ{add(p, b)}

def smile(x: Nat) -> Nat: add(x, "😀")

law add_zero:
  for n: Nat
  Eq(Nat, add(n, 0n), n)

def add_zero(n):
  match n:
    case Zero{}: {==}
    case Succ{p}: {==}

def Nat.double(n: Nat) -> Nat: add(n, n)
