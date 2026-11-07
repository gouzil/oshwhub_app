These LevelDB fixtures were produced by Electron 45.0.0 / Chromium
156.0.8078.48 in an isolated, generated user-data directory on macOS.
They contain only synthetic public theme settings.

The `oshwhub://app` origin saves `dark` / `purple`. A second origin,
`oshwhub://development`, saves `light` / `orange` and must not be imported.
The `deleted` database was reopened after saving, then removed the production
`themeMode` key, leaving both its old record and its deletion in the database.
Only CURRENT, MANIFEST and LevelDB data files are retained; no user profile,
cache, lock or diagnostic log is included.

The Rust migration must preserve the bytes of these inputs. Its LevelDB reader
opens an in-memory copy so recovery and compaction cannot alter the old profile.
