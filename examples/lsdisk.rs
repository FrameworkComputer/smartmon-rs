//! List model, serial and firmware version of all disks (needs root)

fn main() {
    match smartmon::scan_disks() {
        Ok(disks) => {
            for d in disks {
                println!(
                    "{} -d {} [{}]: \"{}\", FW:\"{}\", S/N:\"{}\"",
                    d.name, d.dev_type, d.protocol, d.model, d.firmware, d.serial
                );
            }
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
