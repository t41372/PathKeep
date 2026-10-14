//! Built-in site lists: the category of a whole site, or of one host of it, per regional pack.
//!
//! ## Responsibilities
//! - Name well-known sites, and hosts of them, with the one category all their pages are in.
//!
//! ## Not responsible for
//! - Telling pages of a site apart (the path rules in `rules.rs`), matching (`classification.rs`),
//!   or the user's own overrides, which win over everything here.
//!
//! A site is listed only when its category is plain from the site alone. Portals and apps whose
//! pages span several categories (`yahoo.co.jp`, `naver.com`, `qq.com`) are not listed; a host of
//! theirs that is plain is (`news.naver.com`, `mail.qq.com`). A host is matched before its parents,
//! so `mail.google.com` is work although `google.com` is search. Sites whose pages the path rules
//! tell apart (GitHub, YouTube, Reddit, Amazon) are not listed here: an exact rule would win over
//! those rules (see `every_site_rule_can_fire_and_is_listed_once`).
//!
//! One file per regional pack. Changing a list means changing its pack's version in `rules.rs`,
//! which rebuilds Insights.

mod cn;
mod eu;
mod global;
mod international;
mod jp;
mod kr;
mod ru;
mod tw;
mod us;

pub(super) use self::{
    cn::CN_CORE, eu::EU_CORE, global::GLOBAL_CORE, international::INTERNATIONAL_COMMON,
    jp::JP_CORE, kr::KR_CORE, ru::RU_CORE, tw::TW_CORE, us::US_CORE,
};
