# Local snapshot-reader patch

Upstream: `rusty-leveldb` 4.0.2, MIT, from crates.io. The original license is
retained. OSHWHub uses this only to read an in-memory copy of Electron's old
localStorage on desktop; upstream filesystem support is disabled.

The upstream reader discards some recovery/table errors and assumes valid input.
This copy propagates WAL, MANIFEST and point-lookup errors; validates log
fragments, write-batch counts/tags/lengths, table entries/restarts, filter blocks
and footers; checks table handles against the actual file size before allocating;
and limits a decoded block/logical log record to 64 MiB. MANIFEST string lengths
are checked against the remaining input before copying.
Point lookup also requires a deletion's user key to match the requested key;
a later key's tombstone must not hide an older value in another table.

`Options::automatic_compactions` defaults to true. The migration reader sets it
to false because upstream compaction iterators discard read errors. This patch
does not make upstream writing/compaction suitable for untrusted databases.
The application never iterates or explicitly compacts the imported database.

Regressions cover real Chromium settings and deletion records at the application
layer, damaged checksums/footers, four overlapping L0 tables with a damaged latest
deletion, and malformed records whose checksums are valid. Run library checks:

```sh
CARGO_TARGET_DIR=target/vendor-reader cargo test --manifest-path vendor/rusty-leveldb/Cargo.toml --no-default-features
CARGO_TARGET_DIR=target/vendor-reader cargo test --manifest-path vendor/rusty-leveldb/Cargo.toml --no-default-features --release
```
