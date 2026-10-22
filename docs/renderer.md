# Independent PCB renderer

The PCB preview uses an independent Rust scene compiler and React Native Skia.
Rust owns project download, history authentication and replay, footprint expansion,
geometry, draw batching, and picking. The UI draws the returned SVG paths and
handles the viewport and layer controls. No EasyEDA viewer renderer or closed
`smt-gl-engine` code is imported, linked, copied, or required at runtime.

## Data path

1. Read the public Pro project and its default branch to obtain the history head.
2. Follow parent history IDs until the root or a snapshot. Each history includes
   part zero and every suffix through `num` inclusive. Snapshots use `snapshotUrl`
   and `snapshot_num`, with the snapshot ID as their IV.
3. Authenticate each part with AES-GCM, then gunzip UTF-8 record logs, consuming
   all concatenated gzip members within one output budget and rejecting trailing
   unparsed bytes. Public
   projects tested here use a 128-bit key and 128-bit IV; the decoder also accepts
   256-bit keys and 96-bit IVs. It rejects invalid authentication and oversized
   output.
4. Replay from oldest to newest into documents keyed by UUID. A record key is
   `(document, type, normalized ID)`, including IDs serialized as JSON arrays.
   Higher logical tickets win; equal tickets prefer the lexically smaller client
   ID. Empty bodies remain tombstones. `DELETE_DOC` suppresses deleted documents.
   After the existing per-line whitespace trimming, opaque record headers remain
   as JSON strings during replay, including unknown fields; ticket/client/ID values are extracted for conflict
   resolution without retaining a separate header object tree for each record.
5. Resolve a component's `Footprint` attribute against the replayed project.
   `Device` references can resolve through live device metadata; deleted devices
   cannot restore referenced footprints. Expand footprint
   primitives, attach board `PAD_NET` values, compile paths, and create the picking
   index. No separate external footprint service is required for the checked
   public project.

Requests use the core client's HTTPS host allowlist, response limits, timeouts,
and explicit status/MIME validation. The loader downloads four parts concurrently
while preserving replay order. Decompression, replay, and scene compilation run
on a blocking worker rather than the async network executor.

## Geometry and rendering contract

The scene contains `layers`, `batches`, fallback `texts`, `bounds`, `warnings`, and
load/parse/scene metrics. Batches carry a layer, SVG path, fill/stroke mode, stroke
width, and optional object color. Adjacent compatible paints are combined, with a
512 KiB batch threshold. Skia should use nonzero filling and round stroke joins
and caps. Changing the entire layer to even-odd filling would cancel overlapping
copper.

PCB coordinates stay in document units. Actual PCB examples behave as mil-based
coordinates; the canvas unit is a display preference, so the API does not label
arbitrary raw coordinates as millimeters. The compiler subtracts the canvas
origin and flips Y once. Clients consume scene coordinates directly.

Components apply translation, positive rotation, and a local Y mirror for the
bottom side. The mirror occurs before rotation. Bottom components swap paired
layers 1/2 through 9/10. Polygon pads already contain footprint coordinates;
their copper is not reanchored to the pad center. Hole offset and relative angle
apply to the drill separately.

The implemented primitives include lines, circular arcs, cubic/quadratic paths,
rectangles and circles, pads, vias and drills, polygons/fills, poured copper,
vector images, embedded font outlines, and approximate dimension lines/text.
Visible `ATTR` keys and values use the same embedded-outline or fallback text
path. Board attribute records keep their absolute placement and replace matching
footprint attributes, including hidden overrides. Inherited footprint labels use
the component transform and bottom layer mapping. Empty, malformed, or complex
visible values produce warnings rather than disappearing silently.
`POURED.pourFill` coordinates and widths are scaled by ten; its angles are not.
Stale poured records without a live `POUR` are skipped with a warning. Outlines
alone do not become filled copper.

Closed contours are classified by nesting, then normalized for nonzero filling.
Independent glyph islands remain outer contours and nested contours form holes.
Native circular arcs are retained under board/component transforms; circular
arcs under nonuniform image scaling use bounded polygon approximation. Picking
uses flattened geometry, with 0.05 document-unit arc flattening tolerance before
any nonuniform image scaling, and fixed cubic/quadratic subdivisions.

Layer 47 contains drill masks and is drawn last in `#0b121a`, matching the canvas
background. The UI must keep these colors synchronized. Copper, silkscreen,
outline, and drill layers are enabled by default when the source layer is shown;
mask, paste, and technical layers remain selectable in layer controls.

## Picture ownership

`buildPictures` explicitly releases temporary paths, paints, fonts, and recorders,
and disposes partially built pictures if compilation fails. Once pictures are
published to `Canvas`, its independent render root and recorded commands can
retain them beyond the owning React effect's cleanup. Cleanup must not manually
dispose those published handles: Fast Refresh preserves React state while
restarting effects, so a pending draw could otherwise receive a deleted picture.

Published pictures follow their reachable Skia handles. CanvasKit's Embind
`sk_sp<Picture>` wrappers register with `FinalizationRegistry`; native
`JsiSkPicture` objects release their smart pointers during JSI garbage collection,
scheduling GPU resource release on the creating thread. Browser lifecycle tests
exercise StrictMode, three consecutive Fast Refresh updates, unmounting, real
canvas pixels, and picture finalizers after forced test GC. Collection timing is
not deterministic in normal use, and releasing live resources does not guarantee
an immediate reduction in process RSS/PSS.

## Picking and retained state

`pcb::pick(scene_id, x, y, tolerance, layers)` first queries a 100-unit spatial
grid, then tests actual strokes/filled contours. The optional layer filter must
match the UI's current visible layers. Actual primitive geometry takes priority
over component bounding boxes across all layers. Component candidates use only
their actual drawn layers and return a currently visible layer. Drill masks do
not make an otherwise hidden component selectable. Drill holes are excluded from the pad/via copper hit
geometry. Large objects use a separate bounded candidate list.

The core retains at most two picking scenes, containing owned object IDs,
properties, flattened geometry, the spatial index, and layer metadata. The full
replayed project, including opaque headers, bodies, unknown types, and tombstones,
exists while compiling and is released when the read-only scene is returned.
Eviction yields `scene_expired` on later picks. The current product exposes no
edit/save operations. A future editing entry point must reload and own the full
project state; a picking scene is insufficient to reconstruct the source records.

Releasing the replayed project lowers live retained allocations, but the loader
still downloads all encrypted parts before replaying the complete project in
memory. Compact headers reduce this replay peak; they do not make the loader
streaming. An allocator may retain freed pages, so lower live allocations do not
guarantee an immediate matching reduction in process RSS/PSS. Native memory and
load-time peaks require separate device measurements.

Limits include 256 history versions, 1,024 downloaded parts, 16 MiB per encrypted
part, 128 MiB total download, 256 MiB decompressed history, one million records,
20,000 documents, 128-byte client IDs, 200,000 scene paints, and 64 MiB of generated SVG path data.
Individual geometry is also bounded. Missing parent pages, cycles, malformed
logs, and limit violations return explicit core errors.

## Validation and current limits

Unit fixtures are original miniature records, not copied board artwork. Tests
exercise AES-GCM authentication and gzip limits, chunk/snapshot enumeration,
record conflict rules and tombstones, history pagination, document deletion,
bottom transforms, arcs, rounded pads, pour scaling, nonzero contour topology,
image anchoring, footprint/net resolution, geometry picking, visible layers,
rotated text bounds, zero polygon mask expansion, and visible attribute labels.

The public development-board project
`90602f2f840d4245a97479e90a31d17b`, PCB `d4c95e111c8e5672`, was loaded through the
actual Rust network/decrypt/replay/compiler path on 2026-10-23. Its replay contains
23 histories, 65 encrypted parts, 266 documents, and 282,807 records. All 239
footprint references resolve within the project. These are coverage observations,
not a claim of complete support for every Pro document or mobile performance.
Use the current smoke output for timings and scene totals:

```sh
cargo run -p oshwhub-core --example smoke -- \
  '{"command":"pcb","projectId":"90602f2f840d4245a97479e90a31d17b","documentId":"d4c95e111c8e5672"}'
cargo test -p oshwhub-core pcb::
```

Known gaps emit scene warnings where relevant: special per-layer pad shapes use
the default pad; nonzero polygon pad mask/paste expansions are omitted; hatch
fills show outlines; blind/buried-via spans are approximated as multilayer rings;
reverse text expansion, exact dimension arrows/styles, and external raster
images are unsupported. Text without matching embedded outlines uses the system
font, so its shape and metrics are approximate. Self-intersecting polygon
topology and all producer-version variants have not been exhaustively validated.
Native gesture/device visual validation is separate from these core checks.

## Sources and licensing

- [EasyEDA file-format documentation](https://github.com/easyeda/easyeda-file-format),
  revision `9186c4ac43e67d1bb111ada3f5cc020b76bc543a`: protocol fields and primitive
  semantics. The earlier `easyeda-pro-file-format` URL redirects here. GitHub did
  not report an SPDX license for the documentation at validation time.
- [easyeda-viewer](https://github.com/easyeda/easyeda-viewer), revision
  `1c841a7fdcdc382f7a516a4f5a261ad34cb8162d`, is Apache-2.0. It was
  consulted as a format cross-check; its renderer implementation was not reused.
- Public project responses and encrypted history parts established actual record
  variants. The installed EasyEDA Pro project worker was inspected statically for
  chunk/snapshot protocol behavior; no proprietary implementation was copied.
- The checked source board is published under CC BY-NC-SA 4.0. Downloaded board
  data and generated previews stay outside the repository; they are not test
  fixtures or application assets.
