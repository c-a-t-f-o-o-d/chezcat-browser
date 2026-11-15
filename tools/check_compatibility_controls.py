#!/usr/bin/env python3
"""Mandatory native and harness controls for the bounded Test262 adapter.

Uses the actual existing Rust probe; contains no reference browser substitute.
All ordinary check_compatibility options remain available. No network access.
"""
import sys
from check_compatibility import main

if __name__ == "__main__":
    raise SystemExit(main(["--test262-controls-only", *sys.argv[1:]]))
