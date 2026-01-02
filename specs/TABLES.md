# Oatty CLI — Table Rendering Design

## Default Behavior
- **Array JSON responses** → tables by default.
- `--json` → raw JSON, no table.
- Objects containing arrays → render array.

---

## Column Selection
1. **Schema hints** (preferred).
2. **Built-in presets** for well-known resources:
   - Apps, Releases, Addons, Dynos, Config vars.
3. **Heuristic ranking** of fields:
   - Prefer IDs, names, statuses, timestamps.
   - Penalize long blobs or nested arrays.

---

## Column Header Normalization
- `snake_case` → `Title Case`.
- `_id` → `ID`.
- `_url` → `URL`.
- Preserve acronyms (ID, URL, HTTP).

---

## Value Rendering
- Timestamps → relative + exact.
- Durations → `3m 42s`.
- States → color badges.
- Booleans → ✓ / ✗.
- Secrets → masked by default, reveal opt-in.
- IDs/SHAs → ellipsized (`abcd…1234`).

---

## Usability Features
- `/` search.
- `s` sort by column.
- `c` column picker.
- `Enter` row expand (detail drawer).
- Copy cell or row JSON.
- Presets saved to `~/.config/oatty/ui/presets.json`.

---

## Performance
- Virtualized scrolling for large datasets.
- Lazy pagination (API-driven if supported).
- Minimal redraws for smooth rendering.

---

## Safety
- Auto-redact secret-like keys (`token`, `password`, `api_key`).
- Consistent redaction in tables, details, and logs.
