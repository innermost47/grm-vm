# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Anthony Charretier
"""Times the Python machine of grm-core on a GRML file: python3 tools/bench_python.py GRM_CORE_VM FILE.grm [ROUNDS]"""
import sys
import time

sys.path.insert(0, sys.argv[1])
from language import Network, statement  # noqa: E402
from runtime import Machine  # noqa: E402

net = Network()
for line in open(sys.argv[2]):
    statement(net, line)
t0 = time.perf_counter()
m = Machine(net)
t1 = time.perf_counter()
rounds = m.run(int(sys.argv[3]) if len(sys.argv) > 3 else 100000)
t2 = time.perf_counter()
print(f"python: {len(m.var_names)} nodes, {len(m.factors)} processors; compile {t1 - t0:.3f} s; settle {t2 - t1:.3f} s "
      f"in {rounds} rounds, {m.messages} messages")
