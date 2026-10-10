//! Responsibilities: make up a long search history for benchmarks that need searches at the
//! scale of a large archive (`intelligence::tests::family_scale`, and the worker's 14.4M-visit
//! benchmark): the same searches on every run, in the order they were made.
//! Not responsible for: what a search's visits or pages are, or real search data.
//! Failure modes: a history that is easier than a real one, all different searches with no
//! repeats or rewordings, or words spread so evenly that no word is common; numbers taken on
//! it would then hide the cost of the searches people make most.

/// A small deterministic generator (xorshift64*), so every run makes the same searches.
pub struct Random(pub u64);
impl Random {
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.unit() * n as f64) as usize
    }
}

/// Ranks drawn with Zipf's law (s = 1): a few words come up in many searches.
pub struct Zipf(Vec<f64>);
impl Zipf {
    pub fn new(n: usize) -> Self {
        let mut total = 0.0;
        Self(
            (1..=n)
                .map(|rank| {
                    total += 1.0 / rank as f64;
                    total
                })
                .collect(),
        )
    }
    pub fn sample(&self, random: &mut Random) -> usize {
        let target = random.unit() * self.0.last().unwrap();
        self.0.partition_point(|&cumulative| cumulative < target).min(self.0.len() - 1)
    }
}

/// How many different Latin words the history draws from.
pub const VOCABULARY: usize = 50_000;

/// A made-up Latin word for a rank: syllables, then an `x`, which no stop word ends with.
pub fn made_up_word(rank: usize) -> String {
    const CONSONANTS: &[u8] = b"bcdfghjklmnprstvz";
    const VOWELS: &[u8] = b"aeiou";
    let mut value = rank + 85;
    let mut text = String::new();
    while value > 0 {
        let syllable = value % 85;
        text.push(CONSONANTS[syllable / 5] as char);
        text.push(VOWELS[syllable % 5] as char);
        value /= 85;
    }
    text.push('x');
    text
}

/// Common Chinese characters for searches with no spaces between words.
const HAN: &str = "的一是在不了有和人這中大為上個國我以要他時來用們生到作地於出就分對成會可主發年動同工也能下過子說產種面而方後多定行學法所民得經十三之進著等部度家電力裡如水化高自二理起小物現實加量都兩體制機當使點從業本去把性好應開它合還因由其些然前外天政四日那社義事平形相全表間樣與關各重新線內數正心反你明看原又麼利比或但質氣第向道命此變條只沒結解問意建月公無系軍很情者最立代想已通並提直題黨程展五果料象員革位入常文總次品式活設及管特件長求老頭基資邊流路級少圖山統接知較將組見計別她手角期根論運農指幾九區強放決西被幹做必戰先回則任取據處府研";

/// The first `count` searches of a long history, oldest first: 35% repeat an earlier search
/// (a recent one or a long-lived favourite), 15% reword the search just made (a word added,
/// dropped or replaced), 45% are new searches of one to six words drawn by Zipf's law from
/// `VOCABULARY` words, and 5% are Chinese, with no spaces. A shorter history is the start of a
/// longer one.
pub fn synthetic_searches(count: usize) -> Vec<String> {
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let words = Zipf::new(VOCABULARY);
    let han = HAN.chars().collect::<Vec<_>>();
    let han_rank = Zipf::new(han.len());
    let mut made = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    let mut previous = None::<String>;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let roll = random.unit();
        let query = if roll < 0.35 && !made.is_empty() {
            if random.unit() < 0.5 {
                let recent = made.len().min(200);
                made[made.len() - 1 - random.below(recent)].clone()
            } else {
                made[(made.len() as f64 * random.unit().powi(3)) as usize].clone()
            }
        } else if roll < 0.50 && previous.is_some() {
            let mut parts =
                previous.as_deref().unwrap().split(' ').map(str::to_owned).collect::<Vec<_>>();
            match random.below(3) {
                0 => parts.push(made_up_word(words.sample(&mut random))),
                1 if parts.len() > 1 => {
                    parts.remove(random.below(parts.len()));
                }
                _ => {
                    let at = random.below(parts.len());
                    parts[at] = made_up_word(words.sample(&mut random));
                }
            }
            parts.join(" ")
        } else if roll < 0.95 {
            let length = match random.unit() {
                x if x < 0.15 => 1,
                x if x < 0.45 => 2,
                x if x < 0.70 => 3,
                x if x < 0.85 => 4,
                x if x < 0.95 => 5,
                _ => 6,
            };
            (0..length)
                .map(|_| made_up_word(words.sample(&mut random)))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            (0..2 + random.below(7)).map(|_| han[han_rank.sample(&mut random)]).collect()
        };
        if seen.insert(query.clone()) {
            made.push(query.clone());
        }
        previous = Some(query.clone());
        out.push(query);
    }
    out
}
