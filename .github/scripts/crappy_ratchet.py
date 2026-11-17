#!/usr/bin/env python3
"""Fail when a change makes `cargo crappy` worse than its base.

Usage: crappy_ratchet.py BASE_REPORT HEAD_REPORT

Each report is the full output of `cargo crappy --threshold N`. The crate
does not pass that threshold outright (the browser tree has no host
coverage), so the gate is a ratchet instead: a function may not appear in
the head report unless the base report already has it with a score at least
as high.
"""

import re
import sys

WARNING = re.compile(
    r"^warning\[[^\]]+\]: `(?P<name>.+?)` has .*?\n"
    r" --> (?P<file>\S+?):\d+\n"
    r"(?:.*\n)*?"
    r"\s+= note: CRAPPY = (?P<score>[\d.]+)",
    re.MULTILINE,
)


def scores(path):
    """Map (function, file) to its CRAPPY score for one report."""
    with open(path, encoding="utf-8") as report:
        text = report.read()
    # A build failure prints no analysis and no warnings, which would read
    # as a perfect score. Refuse it.
    if not re.search(r"Analyzed \d+ functions", text):
        sys.exit(f"{path}: cargo crappy did not complete\n{text[-2000:]}")
    return {
        (m["name"], m["file"]): float(m["score"]) for m in WARNING.finditer(text)
    }


def main():
    base, head = scores(sys.argv[1]), scores(sys.argv[2])
    worse = {
        key: (base.get(key), score)
        for key, score in head.items()
        if score > base.get(key, 0.0)
    }
    for (name, path), (before, after) in sorted(worse.items()):
        was = "not flagged" if before is None else f"{before:g}"
        print(f"worse: {name} ({path}): {was} -> {after:g}")
    improved = sum(1 for key, score in base.items() if head.get(key, 0.0) < score)
    print(f"cargo crappy ratchet: {len(worse)} worse, {improved} improved")
    sys.exit(1 if worse else 0)


if __name__ == "__main__":
    main()
