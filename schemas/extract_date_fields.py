#!/usr/bin/env python3
"""
Extract date-like field names from heroku-schema.json.

Detects fields whose schema has format "date-time" or "date", or whose examples
look like ISO dates, by walking properties/definitions recursively.

Usage:
  python3 schemas/extract_date_fields.py [path/to/heroku-schema.json]

Writes a JSON array to stdout, and also updates schemas/date_fields.json next to the schema.
"""
from __future__ import annotations

import json
import os
import re
import sys
from typing import Any, Dict, Iterable, Set


ISO_DATE_RE = re.compile(r"^\d{4}[-/]\d{2}[-/]\d{2}(
    T\d{2}:\d{2}(:\d{2})?(Z|[+-]\d{2}:?\d{2})?
)?$")


def looks_like_iso_date(s: str) -> bool:
    return bool(ISO_DATE_RE.match(s))


def has_date_indicator(node: Any) -> bool:
    if isinstance(node, dict):
        fmt = node.get("format")
        if isinstance(fmt, str) and fmt in ("date-time", "date"):
            return True
        example = node.get("example")
        if isinstance(example, str) and looks_like_iso_date(example):
            return True
        for key in ("anyOf", "oneOf", "allOf"):
            arr = node.get(key)
            if isinstance(arr, list) and any(has_date_indicator(x) for x in arr):
                return True
        items = node.get("items")
        if items and has_date_indicator(items):
            return True
    return False


def collect(node: Any, out: Set[str]) -> None:
    if isinstance(node, dict):
        props = node.get("properties")
        if isinstance(props, dict):
            for k, v in props.items():
                if has_date_indicator(v):
                    out.add(str(k).lower())
                collect(v, out)
        defs = node.get("definitions")
        if isinstance(defs, dict):
            for k, v in defs.items():
                if has_date_indicator(v):
                    out.add(str(k).lower())
                collect(v, out)
        for key in (
            "items",
            "anyOf",
            "oneOf",
            "allOf",
            "not",
            "additionalProperties",
            "patternProperties",
            "targetSchema",
            "schema",
        ):
            val = node.get(key)
            if val is not None:
                collect(val, out)
    elif isinstance(node, list):
        for x in node:
            collect(x, out)


def main() -> int:
    here = os.path.dirname(os.path.abspath(__file__))
    schema_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "heroku-schema.json")
    with open(schema_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    found: Set[str] = set()
    collect(data, found)
    result = sorted(found)

    # Write alongside schema
    out_path = os.path.join(os.path.dirname(schema_path), "date_fields.json")
    with open(out_path, "w", encoding="utf-8") as out:
        json.dump(result, out, indent=2)

    # Also print to stdout
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

