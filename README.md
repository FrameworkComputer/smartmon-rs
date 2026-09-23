# smartmon

Rust bindings for libsmartmon, [smartmontools](https://www.smartmontools.org/)
built as a library.

smartmontools is included as a git submodule and compiled with the `cc` crate,
no autotools needed. `build.rs` generates the headers that `configure` would
normally create (`config.h`, `smartmon/smartmon_config.h`, `smartmon/version.h`).

```
git submodule update --init
cargo build
sudo ./target/debug/examples/lsdisk
```

Currently exposes `scan_disks()`, which returns model, serial and firmware
version of all ATA, NVMe and SCSI disks, including NVMe drives behind USB
bridges.

## Updating smartmontools

After bumping the submodule, check `lib/Makefile.am` (library sources) and the
`os_deps`/`os_libs` section of `configure.ac` for changes and update `build.rs`.

## License

GPL-2.0-or-later, same as smartmontools. See `smartmontools/COPYING`.
