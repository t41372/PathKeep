//! Responsibilities: select pinned built-in files and their HTTP origin. The local fixture
//! branch exists only in debug builds. Not responsible for consent, retries or publication.

pub(super) enum ModelSource {
    Hub,
    #[cfg(debug_assertions)]
    Fixture {
        base: String,
        version: FixtureVersion,
    },
}

impl ModelSource {
    pub(super) fn configured() -> Result<Self, &'static str> {
        #[cfg(debug_assertions)]
        if let Some(value) = std::env::var_os("PATHKEEP_TEST_MODEL_SOURCE") {
            return fixture::source(value.to_str().ok_or("model fixture URL is not UTF-8")?);
        }
        Ok(Self::Hub)
    }

    pub(super) fn url(&self, repo: &str, file: &str) -> String {
        match self {
            Self::Hub => format!("https://huggingface.co/{repo}/resolve/main/{file}"),
            #[cfg(debug_assertions)]
            Self::Fixture { base, .. } => format!("{base}/{file}"),
        }
    }
}

#[cfg(debug_assertions)]
pub(super) enum FixtureVersion {
    V1,
    V2,
}

#[cfg(debug_assertions)]
mod fixture {
    use super::{FixtureVersion, ModelSource};

    pub(super) fn source(base: &str) -> Result<ModelSource, &'static str> {
        const INVALID: &str =
            "expected a loopback model fixture URL such as http://127.0.0.1:8000/v1";
        let base = base.trim_end_matches('/');
        let (authority, version) =
            base.strip_prefix("http://").and_then(|url| url.split_once('/')).ok_or(INVALID)?;
        let address: std::net::SocketAddr = authority.parse().map_err(|_| INVALID)?;
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(INVALID);
        }
        let version = match version {
            "v1" => FixtureVersion::V1,
            "v2" => FixtureVersion::V2,
            _ => return Err(INVALID),
        };
        Ok(ModelSource::Fixture { base: base.to_owned(), version })
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    #[test]
    fn fixture_origin_accepts_only_versioned_loopback_http() {
        let local = fixture::source("http://127.0.0.1:8123/v1").unwrap();
        assert_eq!(local.url("org/model", "weights"), "http://127.0.0.1:8123/v1/weights");
        for url in [
            "https://127.0.0.1:8123/v1",
            "http://192.0.2.1:80/v2",
            "http://127.0.0.1:80/unknown",
            "http://localhost@example.com:80/v1",
        ] {
            assert!(fixture::source(url).is_err(), "{url}");
        }
    }
}
