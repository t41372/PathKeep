//! Injects file and migration faults into closed fixture databases.
//! Expected admission and read outcomes belong to the lifecycle scenarios.

use super::{ArchiveMode, Db, Fixture, PASSWORD, Vault};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Damage {
    FirstPageBit,
    FirstPageGarbage,
    LaterPageGarbage,
    Truncated,
    Header,
    Directory,
    Missing,
    #[cfg(unix)]
    Unreadable,
    Migration,
}

pub(super) const DAMAGE: &[Damage] = &[
    Damage::FirstPageBit,
    Damage::FirstPageGarbage,
    Damage::LaterPageGarbage,
    Damage::Truncated,
    Damage::Header,
    Damage::Directory,
    Damage::Missing,
    #[cfg(unix)]
    Damage::Unreadable,
    Damage::Migration,
];

pub(super) fn spoil(fixture: &Fixture, db: Db, damage: Damage) {
    // Re-run an existing DDL step against its already-created schema. This reaches the real
    // migration transaction in plaintext and encrypted mode, with no injected engine error.
    if matches!(damage, Damage::Migration) {
        let vault = fixture.vault();
        let decoded = fixture.sandbox.path().join("migration-input");
        let file = decoded.join(vault_layout::database_path(db));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        vault
            .store()
            .pause()
            .unwrap()
            .copy_database(db, file.clone(), None, Default::default(), |_| {})
            .unwrap();
        rusqlite::Connection::open(&file)
            .unwrap()
            .execute("DELETE FROM __pk_migrations", ())
            .unwrap();
        let key = (vault.config().archive_mode == ArchiveMode::Encrypted).then(|| {
            *Vault::unlock_data_key(vault.paths(), PASSWORD)
                .unwrap()
                .derive(vault_layout::database_spec(db).key)
                .expose_bytes()
        });
        let mut config = vault_store::Config::new(&decoded);
        config.databases = vec![Db::Scratch, db];
        let input = vault_store::Store::open(config).unwrap();
        let output = fixture.sandbox.path().join("unmigrated.db");
        input
            .pause()
            .unwrap()
            .copy_database(db, output.clone(), key, Default::default(), |_| {})
            .unwrap();
        fixture.session.close_vault().unwrap();
        fs::copy(output, fixture.root.join(vault_layout::database_path(db))).unwrap();
        return;
    }
    // Damage a page used by a real conversation query, independently of the allocation
    // order or number of pages in the fixture. Both root pages hold captured corpus rows.
    let page = if matches!(damage, Damage::LaterPageGarbage) {
        let table = match db {
            Db::AgentHistoryEvidence => "ah_chunks",
            Db::AgentHistoryReadModel => "ah_sessions",
            _ => unreachable!(),
        };
        let page = fixture
            .vault()
            .store()
            .read(db, move |s| {
                s.query_one("SELECT rootpage FROM sqlite_schema WHERE name=?1", [table], |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap();
        assert!(page > 1);
        page as u64
    } else {
        1
    };
    fixture.session.close_vault().unwrap();
    let path = fixture.root.join(vault_layout::database_path(db));
    match damage {
        Damage::FirstPageBit | Damage::FirstPageGarbage | Damage::LaterPageGarbage => {
            overwrite_page(&path, page, matches!(damage, Damage::FirstPageBit));
        }
        Damage::Truncated => fs::write(path, b"bad").unwrap(),
        Damage::Header => {
            fs::OpenOptions::new().write(true).open(path).unwrap().write_all(&[0x55; 16]).unwrap()
        }
        Damage::Directory => {
            fs::remove_file(&path).unwrap();
            fs::create_dir(path).unwrap();
        }
        Damage::Missing => fs::remove_file(path).unwrap(),
        #[cfg(unix)]
        Damage::Unreadable => {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
            assert_eq!(
                fs::File::open(path).unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
        Damage::Migration => {}
    }
}

pub(super) fn overwrite_page(path: &Path, page: u64, flip_one_bit: bool) {
    // Both native formats keep the SQLite page size at offset 16. The first
    // 100 bytes are the database header. Byte 100 belongs to the b-tree.
    let mut file = fs::OpenOptions::new().read(true).write(true).open(path).unwrap();
    let len = file.metadata().unwrap().len();
    file.seek(SeekFrom::Start(16)).unwrap();
    let mut size = [0; 2];
    file.read_exact(&mut size).unwrap();
    let size = match u16::from_be_bytes(size) {
        1 => 65536,
        n => u64::from(n),
    };
    assert!(size >= 512 && size.is_power_of_two() && page * size <= len);
    let skip = if page == 1 { 100 } else { 0 };
    file.seek(SeekFrom::Start((page - 1) * size + skip)).unwrap();
    if flip_one_bit {
        let mut byte = [0];
        file.read_exact(&mut byte).unwrap();
        byte[0] ^= 1;
        file.seek(SeekFrom::Current(-1)).unwrap();
        file.write_all(&byte).unwrap();
    } else {
        file.write_all(&vec![0x55; (size - skip) as usize]).unwrap();
    }
    assert_eq!(file.metadata().unwrap().len(), len);
}
