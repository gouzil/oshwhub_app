NSH 0.4.0 iOS packaging repair
============================

This is the published `nsh-platform-ios` 0.4.0 source (Apache-2.0). Its
`src/lib.rs` includes `../../nsh-platform-macos/src/speech.rs`, which is
absent from the crates.io archive. A clean iOS build fails before the app
is compiled.

The only source change is making `speech` a normal local module, and adding
that file verbatim from NSH revision
`e626a8dd8f8d8b277b9507cfd5ac74d0e67020f3`. `.cargo/config.toml` applies the
repair to this app and NSH's generated runner workspaces. There are no
behavior changes or new framework features. Remove this patch when an NSH
release with a self-contained iOS crate is adopted and tested.
