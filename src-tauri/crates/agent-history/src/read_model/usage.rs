//! Four distinct token categories and the baseline for cumulative meters.
//! Each adapter maps its own producer records to these units.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tokens {
    pub input: i64,
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
}
impl Tokens {
    pub fn total(self) -> i64 {
        self.input + self.cache_write + self.cache_read + self.output
    }
    pub(crate) fn add(&mut self, other: Self) {
        self.input += other.input;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.output += other.output;
    }
    pub(crate) fn difference(self, old: Self) -> Option<Self> {
        let n = Self {
            input: self.input - old.input,
            cache_read: self.cache_read - old.cache_read,
            cache_write: self.cache_write - old.cache_write,
            output: self.output - old.output,
        };
        (n.input >= 0 && n.cache_read >= 0 && n.cache_write >= 0 && n.output >= 0).then_some(n)
    }
}
pub(crate) fn fields(v: &Value, keys: [&str; 4]) -> Option<Tokens> {
    let [i, w, r, o] = keys;
    if ![i, w, r, o].iter().any(|k| v[*k].is_i64()) {
        return None;
    }
    Some(Tokens {
        input: v[i].as_i64().unwrap_or(0),
        cache_write: v[w].as_i64().unwrap_or(0),
        cache_read: v[r].as_i64().unwrap_or(0),
        output: v[o].as_i64().unwrap_or(0),
    })
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Counter {
    pub tokens: Option<Tokens>,
    pub reasoning: Option<i64>,
    pub from_start: bool,
}
