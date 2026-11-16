# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Anthony Charretier
"""A grid of n x n signed nodes, each tied to its neighbours (u = u' ± 1), a few of them observed: the program the
Python and the Rust machines are timed on. Prints it (GRML)."""
import sys

n = int(sys.argv[1])
print("# a grid of", n, "x", n)
for i in range(n):
    for j in range(n):
        print(f"node u_{i}_{j} signed")
for i in range(n):
    for j in range(n):
        if j + 1 < n:
            print(f"relate h_{i}_{j}: u_{i}_{j} - u_{i}_{j + 1} = 0 ± 1")
        if i + 1 < n:
            print(f"relate v_{i}_{j}: u_{i}_{j} - u_{i + 1}_{j} = 0 ± 1")
for k in range(0, n, max(1, n // 4)):
    print(f"observe u_{k}_{k} = {k} ± 0.1")
print(f"observe u_0_{n - 1} = {-n} ± 0.1")
