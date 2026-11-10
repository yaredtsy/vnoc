// GENERATED ONCE by tests/golden/gen_golden.py (an independent Python oracle). Committed as literals.
// Do not edit by hand: if the ID spec changes, that is a new format version, not a test fix.

/// (repo_path, qualname, occurrence, collision counter (0 = none), expected id)
pub const ID_CASES: &[(&str, &str, u32, u32, &str)] = &[
    ("src/orders.py", "OrderService.parse_order", 1, 0, "3RGJ87CMS6TK775XRJV8BW977Y"),
    ("src/orders.py", "OrderService.parse_order", 2, 0, "7KDKFAFR8X61R4QKZJ2VPKCMGY"),
    ("src/orders.py", "OrderService.parse_order", 1, 2, "5AAT7H64W8Z4YDQNACH1MGZ729"),
    ("src/orders.py", "OrderService.parse_order", 1, 3, "5GTJB2R0T3TC8Z0JJDJYFDAPW3"),
    ("src/orders.py", "", 1, 0, "07CAEA84NVXG3MJMSDN81G5VMR"),
    ("src/a.py", "top.<locals>.inner.<locals>.deepest", 1, 0, "06G279GY6KG1KTR82SREQVFTV8"),
    ("my dir/mod.py", "f", 1, 0, "6G2R257878V9JW9YW27ZD0KABW"),
    ("src/big.py", "f", 12, 0, "2SE2PDS3GA34Y5B0ZVWZHASGPH"),
    ("café/é.py", "f", 1, 0, "23HQKHCDR4AC7KJNVCNEWBCAXV"),
    ("café/é.py", "f", 1, 0, "1G4HBT2NW41Z9X8BZA7VDZR8V1"),
];

/// (a 128-bit value, its Crockford text)
pub const ENCODE_CASES: &[(u128, &str)] = &[
    (0, "00000000000000000000000000"),
    (1, "00000000000000000000000001"),
    (31, "0000000000000000000000000Z"),
    (32, "00000000000000000000000010"),
    (42535295865117307932921825928971026431, "0ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
    (42535295865117307932921825928971026432, "10000000000000000000000000"),
    (340282366920938463463374607431768211455, "7ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
    (1512366075204170929049582354406559215, "014D2PF2DBSQQG28T5CY4TQKFF"),
];

/// The repo-relative path the fixture is pretended to live at.
pub const FIXTURE_PATH: &str = "pkg/nested.py";
/// (qualname as Python reports it, occurrence, expected id) for every def and class of the fixture, in source order
pub const FIXTURE_DEFS: &[(&str, u32, &str)] = &[
    ("top", 1, "112D7ZQBQ97JMQK793NWRKBRG8"),
    ("top.<locals>.inner", 1, "0T77ZWM1PJA45Q7TRKN825HN9M"),
    ("top.<locals>.inner.<locals>.deepest", 1, "328FR34V0ACY5YAX0NZGFHAAT1"),
    ("top.<locals>.Local", 1, "25NDN9QEW1A112N72D2BV2CACZ"),
    ("top.<locals>.Local.method", 1, "6PSH38QGYNTKFT7TGFQH6CAXSJ"),
    ("Outer", 1, "1B3RCD4M0MWCJ231D0Z9CKS2QG"),
    ("Outer.method", 1, "4JNVH7SVFHW0N2HMH1DBQCK77M"),
    ("Outer.Nested", 1, "2FEZ0VE6G0JFP4PPZQ3PXNFQQF"),
    ("Outer.Nested.method", 1, "3VVM1MVXXK9DTNR97ARPGZSB1E"),
    ("Outer.method", 2, "5R71779VA2M1WZBMGKFQS96DV7"),
    ("top", 2, "7KZJCW482SGX1E7GV8JGSCFNSV"),
    ("cond", 1, "53PE6PPWZ5SMJZGHHKQTMJER50"),
    ("cond", 2, "5T0Y3WEFKD87JADAE57B5SDBB9"),
    ("decorated", 1, "4SV036EYAZHDHJDZWQ70Z90D1B"),
    ("coro", 1, "00G2XAWVJBNVASGK89JCPAACBM"),
];
