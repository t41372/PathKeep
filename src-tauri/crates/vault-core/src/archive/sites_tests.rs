//! Responsibilities: pages written under another public suffix list are split again, once.
//! Not responsible for: the split itself (`visit_taxonomy::tests`).
//! Failure modes (from `sites.rs`): pages left under the old split; the walk repeated on every
//! open; a page whose site did not change rewritten.
use super::*;
use vault_store::{Db, Priority};

fn sites(vault: &Vault) -> Vec<(String, Option<String>)> {
    vault
        .store()
        .read(Db::Archive, |s| {
            s.query("SELECT url,registrable_domain FROM pages ORDER BY id", (), |r| {
                Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?))
            })
        })
        .unwrap()
}

#[test]
fn pages_split_by_an_older_list_are_split_again_once() {
    for key in [None, Some([7; 32])] {
        let vault = Vault::open_temp_with_key(key).expect("open");
        // Written by a build whose list had no private section: one site for all of github.io.
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                for (url, site) in [
                    ("https://rust-lang.github.io/async-book/", Some("github.io")),
                    ("https://tokio-rs.github.io/", Some("github.io")),
                    ("https://www.bbc.co.uk/news", Some("bbc.co.uk")),
                    // Not a web page: no site, as the app writes it.
                    ("chrome-extension://abcdefghijklmnop/popup.html", Some("")),
                ] {
                    s.execute(
                        "INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?1)",
                        vault_store::params![url, site],
                    )?;
                }
                Ok(())
            })
            .unwrap();

        assert_eq!(resplit_if_list_changed(&vault).unwrap(), 2, "only the github.io pages move");
        assert_eq!(
            sites(&vault),
            [
                (
                    "https://rust-lang.github.io/async-book/".into(),
                    Some("rust-lang.github.io".into())
                ),
                ("https://tokio-rs.github.io/".into(), Some("tokio-rs.github.io".into())),
                ("https://www.bbc.co.uk/news".into(), Some("bbc.co.uk".into())),
                ("chrome-extension://abcdefghijklmnop/popup.html".into(), Some(String::new())),
            ]
        );

        // Recorded: the next open reads nothing, so a value put there by hand stays.
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("UPDATE pages SET registrable_domain='marker' WHERE id=3", ())?;
                Ok(())
            })
            .unwrap();
        assert_eq!(resplit_if_list_changed(&vault).unwrap(), 0);
        assert_eq!(sites(&vault)[2].1.as_deref(), Some("marker"));
    }
}
