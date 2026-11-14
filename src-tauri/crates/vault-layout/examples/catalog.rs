//! Responsibilities: print a reproducible reference of every declared policy row.
//! Not responsible for: inspecting an archive or producing test expectations.
use vault_layout::*;

fn main() {
    println!(
        "| Artifact | Path rule | Class | Scope | Database | File key | Open | Snapshot | Restore | Portable | Mode change | At rest | Wipe | Usage | Retention | Health | Export target | Conversion | Blocks new envelope | Password change | Move import |"
    );
    println!(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for row in artifacts() {
        let p = row.lifecycle;
        let cells = [
            format!("{:?}", row.id),
            format!("{:?}", row.path),
            format!("{:?}", row.content),
            format!("{:?}", row.scope),
            format!("{:?}", row.database),
            format!("{:?}", row.file_key),
            format!("{:?}", p.open),
            format!("{:?}", p.snapshot),
            format!("{:?}", p.restore),
            format!("{:?}", p.portable),
            format!("{:?}", p.mode_change),
            format!("{:?}", p.at_rest),
            format!("{:?}", p.wipe),
            format!("{:?}", p.usage),
            format!("{:?}", p.retention),
            format!("{:?}", p.health),
            format!("{:?}", p.export_target),
            format!("{:?}", p.conversion),
            format!("{:?}", p.prevents_new_envelope),
            format!("{:?}", p.password_change),
            format!("{:?}", p.move_import),
        ];
        println!("| {} |", cells.join(" | "));
    }
}
