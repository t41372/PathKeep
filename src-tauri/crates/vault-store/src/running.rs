//! Responsibilities: name in the log a statement that has been running for a long time, again
//! each time that time doubles, and once more when it ends.
//! Not responsible for: stopping it (the SDK has no interrupt), timing statements that end
//! soon, work made of many short statements, or what the store runs for itself (BEGIN and
//! COMMIT, checkpoints, a migration's bookkeeping, a query plan, copying a database).
//! Failure modes: a statement runs for hours and the log says nothing (2026-11-13); the line
//! carries a value written into the statement; every statement pays for the watch; one long
//! statement fills the log; the watcher outlives the store. Tests: tests/long_statement.rs.
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Bytes of a statement kept for the log line. Copied on every statement, so kept short.
const SHOWN: usize = 300;

struct Statement {
    since: Instant,
    /// The age at which the watcher next names it.
    next: Duration,
    named: bool,
}
#[derive(Default)]
struct Now {
    statement: Option<Statement>,
    /// The start of the statement's text, as written. Reused, so a statement allocates nothing.
    sql: String,
    /// Whether the text went on past what `sql` keeps.
    cut: bool,
}

/// What one connection is running now.
pub(crate) struct Slot {
    label: String,
    after: Duration,
    now: Mutex<Now>,
}
/// Held while a statement runs.
pub(crate) struct Running<'a>(Option<&'a Slot>);

impl Slot {
    /// A statement begun while this connection is already inside one (a row callback that
    /// queries again) belongs to the outer one and is not watched on its own.
    pub fn begin(&self, sql: &str) -> Running<'_> {
        let mut now = self.now.lock().unwrap();
        if now.statement.is_some() {
            return Running(None);
        }
        let mut end = sql.len().min(SHOWN);
        while !sql.is_char_boundary(end) {
            end -= 1;
        }
        now.sql.clear();
        now.sql.push_str(&sql[..end]);
        now.cut = end < sql.len();
        now.statement = Some(Statement { since: Instant::now(), next: self.after, named: false });
        Running(Some(self))
    }
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        let Some(slot) = self.0 else { return };
        let ended = slot.now.lock().unwrap().statement.take();
        if let Some(statement) = ended.filter(|statement| statement.named) {
            log::warn!(
                "{} finished the statement it had been running for {}",
                slot.label,
                seconds(statement.since.elapsed())
            );
        }
    }
}

struct Shared {
    slots: Mutex<Vec<Arc<Slot>>>,
    stopped: Mutex<bool>,
    wake: Condvar,
    after: Duration,
}
impl Shared {
    fn name_long_statements(&self) {
        for slot in self.slots.lock().unwrap().iter() {
            let mut now = slot.now.lock().unwrap();
            let Now { statement: Some(statement), sql, cut } = &mut *now else { continue };
            let age = statement.since.elapsed();
            if age < statement.next {
                continue;
            }
            statement.next *= 2;
            statement.named = true;
            log::warn!(
                "{} has been running one statement for {}: {}",
                slot.label,
                seconds(age),
                without_values(sql, *cut)
            );
        }
    }
}

/// Owns the thread that looks at every connection's statement a few times per `after`.
pub(crate) struct Watch {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}
impl Watch {
    pub fn start(after: Duration) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            slots: Mutex::default(),
            stopped: Mutex::new(false),
            wake: Condvar::new(),
            after,
        });
        let watched = shared.clone();
        // Often enough that the first line is not much later than `after`, and no more often
        // than every few seconds in the app, where `after` is half a minute.
        let every = (after / 6).clamp(Duration::from_millis(10), Duration::from_secs(5));
        let thread = thread::Builder::new().name("store-watch".into()).spawn(move || {
            let mut stopped = watched.stopped.lock().unwrap();
            while !*stopped {
                stopped = watched.wake.wait_timeout(stopped, every).unwrap().0;
                if !*stopped {
                    watched.name_long_statements();
                }
            }
        })?;
        Ok(Self { shared, thread: Some(thread) })
    }
    /// A place for one more connection to say what it is running.
    pub fn slot(&self, label: String) -> Arc<Slot> {
        let slot = Arc::new(Slot { label, after: self.shared.after, now: Mutex::default() });
        self.shared.slots.lock().unwrap().push(slot.clone());
        slot
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        *self.shared.stopped.lock().unwrap() = true;
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn seconds(age: Duration) -> String {
    if age < Duration::from_secs(10) {
        format!("{:.1} s", age.as_secs_f64())
    } else {
        format!("{} s", age.as_secs())
    }
}

/// The statement on one line, with what stands between quotes replaced by `?` and comments
/// left out: bound values are never in the text, but a statement put together with a value
/// in it must not put that value in the log. Quotes and comments are told apart the way
/// `sql::tokens` does it, since an apostrophe in a comment is not the start of a literal.
/// Whatever is still open where the text ends is dropped with it.
fn without_values(sql: &str, cut: bool) -> String {
    let mut shown = String::with_capacity(sql.len());
    let mut space = false;
    let mut rest = sql.chars().peekable();
    while let Some(c) = rest.next() {
        let closer = match c {
            '\'' | '"' | '`' => Some(c),
            '[' => Some(']'),
            _ => None,
        };
        if let Some(closer) = closer {
            let _ = rest.by_ref().find(|inside| *inside == closer);
            if std::mem::take(&mut space) {
                shown.push(' ');
            }
            shown.push('?');
        } else if c == '-' && rest.peek() == Some(&'-') {
            let _ = rest.by_ref().find(|inside| *inside == '\n');
            space = !shown.is_empty();
        } else if c == '/' && rest.peek() == Some(&'*') {
            rest.next();
            let mut star = false;
            for inside in rest.by_ref() {
                if star && inside == '/' {
                    break;
                }
                star = inside == '*';
            }
            space = !shown.is_empty();
        } else if c.is_whitespace() {
            space = !shown.is_empty();
        } else {
            if std::mem::take(&mut space) {
                shown.push(' ');
            }
            shown.push(c);
        }
    }
    if cut {
        shown.push('…');
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_between_quotes_never_reaches_the_line() {
        let written = "SELECT  id\n FROM visits WHERE url='https://a.example/it''s' AND \"col\">1";
        assert_eq!(without_values(written, false), "SELECT id FROM visits WHERE url=?? AND ?>1");
        // Cut inside a literal: the rest of the kept text is the value and stays out.
        assert_eq!(
            without_values("SELECT 1 WHERE note='half a secr", true),
            "SELECT 1 WHERE note=?…"
        );
    }
    #[test]
    fn a_quote_in_a_comment_does_not_turn_the_literals_after_it_inside_out() {
        // The validator lets comments through (`sql::tokens`), and the app's statements have
        // them. An apostrophe in one used to open a literal that the next real quote closed.
        assert_eq!(
            without_values(
                "SELECT /* user's note */ 'private-value' FROM t -- it's\n WHERE a=[it's] AND b=`c's`",
                false
            ),
            "SELECT ? FROM t WHERE a=? AND b=?"
        );
        assert_eq!(without_values("SELECT 1 /* user's 'private", true), "SELECT 1…");
        assert_eq!(without_values("SELECT 2 -- user's 'private", true), "SELECT 2…");
    }
}
