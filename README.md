# lidar-dataset-generator
## Building presets

```powershell
cargo run -p buildings-generator
cargo run -p buildings-generator -- --preset l-shaped
cargo run -p buildings-generator -- --preset annex
```

Press `P` to cycle Rectangular / L-shaped / House with annex. `R` generates
another building within the selected preset; `Shift+R` repeats the same seed.
Compound presets use flat roofs: L-shaped wings have equal heights, while the
annex is lower than the main volume. Shared walls are excluded from the point
cloud. The occupancy bounds cover the complete footprint.

```powershell
cargo test -p buildings-generator --lib
```

Tall-building presets (also available with `P`):

```powershell
cargo run -p buildings-generator -- --preset apartment
cargo run -p buildings-generator -- --preset skyscraper
```

Apartment blocks have 6?16 floors; skyscrapers have 32?60 floors, larger windows
and no balconies. Both use flat roofs with parapets and aligned window rows.
