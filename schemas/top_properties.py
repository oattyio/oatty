import json
import sys
from collections import Counter
from typing import Any, Dict, Set


def json_pointer_get(doc: Any, pointer: str) -> Any:
    """Resolve a JSON Pointer against the given document.

    Supports RFC 6901 escaping (~0 => ~, ~1 => /).
    Pointer should start with '#/' or '/'. If it starts with '#', the '#'
    will be stripped.
    """
    if pointer.startswith('#'):
        pointer = pointer[1:]
    if pointer == '':
        return doc
    if not pointer.startswith('/'):
        raise ValueError(f"Unsupported $ref (non-local): {pointer}")
    parts = pointer.split('/')[1:]
    cur = doc
    for raw in parts:
        token = raw.replace('~1', '/').replace('~0', '~')
        if isinstance(cur, list):
            try:
                idx = int(token)
            except ValueError:
                raise KeyError(f"List index not integer in pointer: {raw}")
            cur = cur[idx]
        elif isinstance(cur, dict):
            if token not in cur:
                raise KeyError(f"Pointer token not found: {token}")
            cur = cur[token]
        else:
            raise KeyError(f"Cannot dereference token '{raw}' on non-container")
    return cur


def count_properties_in_target_payloads(doc: Dict[str, Any]) -> Counter:
    counts: Counter = Counter()

    def walk(node: Any, visited: Set[int]):
        # Avoid cycles on dict/list nodes within a single payload root
        if isinstance(node, (dict, list)):
            node_id = id(node)
            if node_id in visited:
                return
            visited.add(node_id)
        else:
            return

        if isinstance(node, dict):
            # Resolve $ref first if local
            ref = node.get('$ref')
            if isinstance(ref, str):
                if ref.startswith('#') or ref.startswith('/'):
                    try:
                        target = json_pointer_get(doc, ref)
                        walk(target, visited)
                    except Exception:
                        # Ignore unresolved pointers but continue traversal of siblings
                        pass
                # Continue to walk other keys as some schemas combine $ref and constraints

            # Count property names under "properties"
            props = node.get('properties')
            if isinstance(props, dict):
                for prop_name, prop_schema in props.items():
                    counts[prop_name] += 1
                    walk(prop_schema, visited)

            # Recurse into other potential sub-schemas inside the payload schema
            for key, value in node.items():
                if key in ('properties',):
                    continue  # already handled
                if key in ('patternProperties',):
                    if isinstance(value, dict):
                        for sub in value.values():
                            walk(sub, visited)
                    continue
                if key in (
                    'items', 'additionalItems', 'additionalProperties', 'propertyNames',
                    'definitions', 'dependencies', 'allOf', 'anyOf', 'oneOf', 'not',
                    'if', 'then', 'else', 'contains'
                ):
                    walk(value, visited)
                else:
                    if isinstance(value, (dict, list)):
                        walk(value, visited)
        elif isinstance(node, list):
            for item in node:
                walk(item, visited)

    # Find all target payload schemas under definitions' links (hyper-schema links[].targetSchema)
    payload_schemas: list = []
    definitions = doc.get('definitions')
    if isinstance(definitions, dict):
        for _def_name, _def in definitions.items():
            if not isinstance(_def, dict):
                continue
            links = _def.get('links')
            if isinstance(links, list):
                for link in links:
                    if not isinstance(link, dict):
                        continue
                    ts = link.get('targetSchema')
                    if isinstance(ts, (dict, list)):
                        payload_schemas.append(ts)

    # Traverse each payload schema separately so repeated refs count per use
    for payload in payload_schemas:
        walk(payload, set())

    return counts


def main():
    path = 'heroku-schema.json'
    if len(sys.argv) > 1:
        path = sys.argv[1]
    with open(path, 'r', encoding='utf-8') as f:
        doc = json.load(f)
    counts = count_properties_in_target_payloads(doc)
    top = counts.most_common(50)
    for name, cnt in top:
        print(f"{name}\t{cnt}")


if __name__ == '__main__':
    main()
