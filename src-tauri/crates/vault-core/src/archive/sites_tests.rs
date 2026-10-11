//! Responsibilities: pages written under another public suffix list are split again, once, and
//! whole-site stars keep covering the pages they covered.
//! Not responsible for: the split itself (`visit_taxonomy::tests`).
//! Failure modes (from `sites.rs`): pages left under the old split; the walk repeated on every
//! open; a page whose site did not change rewritten. For stars on a site the new list split
//! apart (`github.io`): the star left matching nothing; a site under it left uncovered while the
//! star stays because its own site still has pages (`amazonaws.com` over
//! `bucket.s3.amazonaws.com`); a site that only ends in the same letters starred (`notgithub.io`);
//! a star the user put on one of the new sites given the old star's time; a star with no page
//! under it deleted; page stars or unaffected site stars touched; a second run changing anything.
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

fn stars(vault: &Vault) -> Vec<(String, String, String, Option<String>)> {
    vault
        .store()
        .read(Db::Archive, |s| {
            s.query(
                "SELECT entity_kind,entity_key,starred_at,source_profile FROM star
                 ORDER BY entity_kind,entity_key",
                (),
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
        })
        .unwrap()
}

#[test]
fn a_whole_site_star_keeps_covering_the_sites_it_covered() {
    for key in [None, Some([9; 32])] {
        let vault = Vault::open_temp_with_key(key).expect("open");
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                // Split by a list without the private section, as earlier releases split them.
                for (url, site) in [
                    ("https://rust-lang.github.io/async-book/", "github.io"),
                    ("https://tokio-rs.github.io/", "github.io"),
                    ("https://notgithub.io/", "notgithub.io"),
                    ("https://amazonaws.com/", "amazonaws.com"),
                    ("https://bucket.s3.amazonaws.com/report.csv", "amazonaws.com"),
                    ("https://www.bbc.co.uk/news", "bbc.co.uk"),
                ] {
                    s.execute(
                        "INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?1)",
                        vault_store::params![url, site],
                    )?;
                }
                for (kind, key, at, profile) in [
                    ("domain", "github.io", "2026-01-01T00:00:00Z", Some("chrome:Default")),
                    ("domain", "tokio-rs.github.io", "2025-06-01T00:00:00Z", None),
                    ("domain", "amazonaws.com", "2026-02-01T00:00:00Z", None),
                    ("domain", "gone.example", "2026-03-01T00:00:00Z", None),
                    ("domain", "bbc.co.uk", "2026-04-01T00:00:00Z", None),
                    ("url", "https://tokio-rs.github.io/", "2026-05-01T00:00:00Z", None),
                ] {
                    s.execute(
                        "INSERT INTO star(entity_kind,entity_key,starred_at,source_profile)
                         VALUES(?1,?2,?3,?4)",
                        vault_store::params![kind, key, at, profile],
                    )?;
                }
                Ok(())
            })
            .unwrap();

        resplit_if_list_changed(&vault).unwrap();
        let star = |kind: &str, key: &str, at: &str, profile: Option<&str>| {
            (kind.to_owned(), key.to_owned(), at.to_owned(), profile.map(str::to_owned))
        };
        let expected = vec![
            star("domain", "amazonaws.com", "2026-02-01T00:00:00Z", None),
            star("domain", "bbc.co.uk", "2026-04-01T00:00:00Z", None),
            star("domain", "bucket.s3.amazonaws.com", "2026-02-01T00:00:00Z", None),
            star("domain", "gone.example", "2026-03-01T00:00:00Z", None),
            star("domain", "rust-lang.github.io", "2026-01-01T00:00:00Z", Some("chrome:Default")),
            star("domain", "tokio-rs.github.io", "2025-06-01T00:00:00Z", None),
            star("url", "https://tokio-rs.github.io/", "2026-05-01T00:00:00Z", None),
        ];
        assert_eq!(stars(&vault), expected);

        // Carrying them again changes nothing.
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("DELETE FROM archive_meta", ())?;
                Ok(())
            })
            .unwrap();
        resplit_if_list_changed(&vault).unwrap();
        assert_eq!(stars(&vault), expected);
    }
}
