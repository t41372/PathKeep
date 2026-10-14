# Public Suffix List

`public_suffix_list.dat` is Mozilla's [Public Suffix List](https://publicsuffix.org/), both
sections (ICANN and private), as fetched from https://publicsuffix.org/list/public_suffix_list.dat.
It is under the Mozilla Public License 2.0 (see the file's header).

PathKeep reads it at build time (`visit_taxonomy/url.rs`) to split hosts into sites the way
browsers do: `rust-lang.github.io` and `tokio-rs.github.io` are two sites, `news.bbc.co.uk` and
`www.bbc.co.uk` one.

Refresh it before a release with `bun run update:public-suffix-list`. Its `// VERSION:` line is
part of the URL rules' version, so Insights are built anew, and an archive whose pages were split
by an older list splits them again when it opens (`archive/sites.rs`).
