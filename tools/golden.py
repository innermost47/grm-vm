# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Anthony Charretier
"""Writes the values the Python machine of grm-core gives on the parity cases (tests/parity/cases.json), for the
Rust engine to find again. Run from anywhere:

    PYTHONPATH=/path/to/pygrm/src python3 tools/golden.py /path/to/grm-core/vm
"""
import json
import math
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, sys.argv[1])

import i18n  # noqa: E402
from language import Network, statement  # noqa: E402
from runtime import Machine  # noqa: E402

i18n.set_language("en") if hasattr(i18n, "set_language") else None


def number(v):
    return None if v is None or (isinstance(v, float) and not math.isfinite(v)) else v


def state(m, net):
    out = {}
    for n in net.nodes:
        out[n] = number(m.value_si(n))
    return out


cases = json.load(open(os.path.join(here, "..", "tests", "parity", "cases.json")))
golden = {}
for case in cases:
    net = Network()
    for line in case["lines"]:
        statement(net, line)
    m = Machine(net)
    ticks = []
    for action in case["actions"]:
        kind = action[0]
        if kind == "tick":
            m.tick(max_rounds=1000)
            ticks.append(state(m, net))
        elif kind == "ticks":
            for _ in range(action[1]):
                m.tick(max_rounds=1000)
                ticks.append(state(m, net))
        elif kind == "observe":
            m.observe(action[1], action[2], action[3])
    golden[case["name"]] = {"ticks": ticks, "rounds": m.rounds}
json.dump(golden, open(os.path.join(here, "..", "tests", "parity", "golden.json"), "w"), indent=1)
print("golden values for", len(golden), "cases")
