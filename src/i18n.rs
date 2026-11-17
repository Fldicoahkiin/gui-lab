//! Text in more than one language (DESIGN.md §1.20). Every string a person reads comes from a message
//! catalog: one plain text file per locale (`assets/i18n/<tag>.txt`), keys to text with named arguments
//! and plural forms. The locale lives in the application's state; the runtime makes it the current one
//! while the view is built (`Ui::on_locale`), so switching is an ordinary message and everything is laid
//! out again (the text cache is keyed by the strings themselves).
//!
//! The format, one message per line:
//!
//! ```text
//! # a comment
//! dialog.close = Close
//! list.picked = Picked: {row}
//! files.one = {n} file
//! files.other = {n} files
//! long.text = A first line
//!   and a second one (continuation lines start with two spaces)
//! ```
//!
//! `{name}` is replaced by the argument `name` (numbers are written the locale's way); `{{` is a brace.
//! A key with `.one`, `.other`… forms is plural: the form is chosen by the argument `n` (or the first
//! number given) with CLDR's cardinal rules (`intl_pluralrules`). A key missing in a locale falls back
//! along the chain (zh-Hans → en); missing everywhere, `try_tr` says so (`I18nError`) and `tr` shows
//! the key in brackets and remembers it (`misses`), so tests can list what is missing.
//!
//! The framework's catalogs hold its components' own text (`widget.*`). An application ships its own
//! catalogs in the same format and installs them once at start (`install`); its keys are looked up
//! first, then the framework's.
use crate::error::I18nError;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::OnceLock;

/// A language. The framework ships catalogs for `ZhHans`, `En` and `Locale::AR` (Arabic, right to
/// left); any other BCP 47 tag is an `Other` locale that an application gives catalogs for
/// (`install`), falling back to English.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Locale {
    /// Simplified Chinese, the default.
    #[default]
    ZhHans,
    En,
    /// Any other language, by its tag.
    Other(Tag),
}
/// Which way a locale's lines run (the layout's row direction follows it: `El::rtl`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction { Ltr, Rtl }

/// A BCP 47 language tag of up to 15 ASCII letters, digits and hyphens (`ar`, `pt-BR`, `sr-Latn`),
/// kept inline so a `Locale` stays `Copy`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag { b: [u8; 15], n: u8 }
impl Tag {
    /// The tag, if it is one: a language subtag of two or three letters, then subtags of letters and
    /// digits separated by hyphens (`_` is read as `-`).
    pub const fn new(t: &str) -> Option<Tag> {
        let s = t.as_bytes();
        if s.is_empty() || s.len() > 15 { return None; }
        let mut b = [0u8; 15];
        let mut i = 0;
        let mut first = 0;
        let mut in_first = true;
        while i < s.len() {
            let c = s[i];
            let c = if c == b'_' { b'-' } else { c };
            if c == b'-' {
                if i == 0 || i + 1 == s.len() || b[i - 1] == b'-' { return None; }
                in_first = false;
            } else if c.is_ascii_alphabetic() {
                if in_first { first += 1; }
            } else if !(c.is_ascii_digit() && !in_first) {
                return None;
            }
            b[i] = c;
            i += 1;
        }
        if first < 2 || first > 3 { return None; }
        Some(Tag { b, n: s.len() as u8 })
    }
    pub fn as_str(&self) -> &str { std::str::from_utf8(&self.b[..self.n as usize]).unwrap_or("und") }
    /// The language subtag, lower case (`pt` for `pt-BR`).
    fn language(&self) -> String { self.as_str().split('-').next().unwrap_or("").to_ascii_lowercase() }
}
impl std::fmt::Debug for Tag { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) } }

/// The catalogs, as shipped (compiled in, so the web build has them too).
const FILES: [(Locale, &str, &str); 3] = [
    (Locale::ZhHans, "assets/i18n/zh-Hans.txt", include_str!("../assets/i18n/zh-Hans.txt")),
    (Locale::En, "assets/i18n/en.txt", include_str!("../assets/i18n/en.txt")),
    (Locale::AR, "assets/i18n/ar.txt", include_str!("../assets/i18n/ar.txt")),
];

/// Languages written right to left (by language subtag; a script subtag decides first).
const RTL: [&str; 10] = ["ar", "he", "fa", "ur", "ps", "yi", "dv", "ckb", "sd", "ug"];

impl Locale {
    /// Arabic.
    pub const AR: Locale = match Tag::new("ar") { Some(t) => Locale::Other(t), None => Locale::En };
    /// The locales the framework ships catalogs for.
    pub const ALL: [Locale; 3] = [Locale::ZhHans, Locale::En, Locale::AR];
    /// Its BCP 47 tag.
    pub fn tag(&self) -> &str { match self { Locale::ZhHans => "zh-Hans", Locale::En => "en", Locale::Other(t) => t.as_str() } }
    /// The locale for a tag: Chinese and English (any region) are the built-in ones; any other
    /// well-formed tag is `Other`, matched to a shipped or installed catalog by tag or by language
    /// (`ar-EG` finds `ar`). `None` only for something that is not a tag.
    pub fn from_tag(tag: &str) -> Option<Locale> {
        let t = tag.trim().to_ascii_lowercase().replace('_', "-");
        if t.starts_with("zh") { return Some(Locale::ZhHans); }
        if t == "en" || t.starts_with("en-") { return Some(Locale::En); }
        // The usual case: language lower, script title, region upper (`sr-Latn-RS`).
        let canon: Vec<String> = t.split('-').enumerate().map(|(i, p)| match (i, p.len()) {
            (0, _) => p.to_owned(),
            (_, 4) => p[..1].to_ascii_uppercase() + &p[1..],
            (_, 2) => p.to_ascii_uppercase(),
            _ => p.to_owned(),
        }).collect();
        let new = Tag::new(&canon.join("-"))?;
        let lang = new.language();
        let known = || FILES.iter().map(|f| f.0).chain(APP.get().into_iter().flatten().map(|c| c.0));
        known().find(|l| l.tag().eq_ignore_ascii_case(&t))
            .or_else(|| known().find(|l| matches!(l, Locale::Other(k) if k.language() == lang && !k.as_str().contains('-'))))
            .or(Some(Locale::Other(new)))
    }
    /// Its name in itself (for a language switcher): the catalog's `locale.name`, else the tag.
    pub fn name(&self) -> String {
        match self {
            Locale::ZhHans => "简体中文".into(),
            Locale::En => "English".into(),
            Locale::Other(_) => lookup(*self, "locale.name", &[]).unwrap_or_else(|| self.tag().to_owned()),
        }
    }
    pub fn direction(&self) -> Direction {
        let t = self.tag().to_ascii_lowercase();
        let mut parts = t.split('-');
        let lang = parts.next().unwrap_or("");
        for p in parts {
            if p.len() == 4 { return if matches!(p, "arab" | "hebr" | "thaa" | "syrc" | "nkoo" | "adlm" | "rohg") { Direction::Rtl } else { Direction::Ltr }; }
        }
        if RTL.contains(&lang) { Direction::Rtl } else { Direction::Ltr }
    }
    /// Where a missing key is looked for next.
    pub fn fallback(&self) -> Option<Locale> { match self { Locale::ZhHans | Locale::Other(_) => Some(Locale::En), Locale::En => None } }
    /// The plural category of `n` here (`"one"`, `"other"`…).
    pub fn plural(&self, n: f64) -> &'static str {
        use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
        thread_local! { static RULES: RefCell<HashMap<Locale, PluralRules>> = RefCell::new(HashMap::new()); }
        RULES.with(|r| {
            let mut r = r.borrow_mut();
            let rules = r.entry(*self).or_insert_with(|| {
                let lang = match self { Locale::ZhHans => "zh".to_owned(), Locale::En => "en".to_owned(), Locale::Other(t) => t.language() };
                let id: unic_langid::LanguageIdentifier = lang.parse().unwrap_or_default();
                PluralRules::create(id, PluralRuleType::CARDINAL).unwrap_or_else(|_| PluralRules::create(unic_langid::LanguageIdentifier::default(), PluralRuleType::CARDINAL).expect("root plural rules"))
            });
            match rules.select(n) {
                Ok(PluralCategory::ZERO) => "zero", Ok(PluralCategory::ONE) => "one", Ok(PluralCategory::TWO) => "two",
                Ok(PluralCategory::FEW) => "few", Ok(PluralCategory::MANY) => "many", _ => "other",
            }
        })
    }
    /// The plural categories this locale's catalog must give for a plural key.
    /// (the forms CLDR's rules can choose, found by asking them for a range of counts).
    pub fn categories(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = vec![];
        let ns = (0..=200).map(f64::from).chain([1000.0, 1_000_000.0, 0.5, 1.5]);
        for n in ns { let c = self.plural(n); if !out.contains(&c) { out.push(c); } }
        const ORDER: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];
        out.sort_by_key(|c| ORDER.iter().position(|o| o == c));
        out
    }

    /// The text for `key`, or (missing everywhere) the key in brackets.
    pub fn tr(&self, key: &str) -> String { self.tr_args(key, &[]) }
    /// The text for `key` with named arguments.
    pub fn tr_args(&self, key: &str, args: &[(&str, Arg)]) -> String {
        self.try_tr(key, args).unwrap_or_else(|_| { MISSES.with(|m| { m.borrow_mut().insert(key.to_owned()); }); format!("⟨{key}⟩") })
    }
    /// The text for `key`, along the fallback chain, or what is missing.
    pub fn try_tr(&self, key: &str, args: &[(&str, Arg)]) -> Result<String, I18nError> {
        let me = *self;
        let mut at = Some(me);
        while let Some(l) = at {
            if let Some(s) = lookup(l, key, args) { return Ok(fill(l, &s, args)); }
            at = l.fallback();
        }
        let chain: Vec<String> = std::iter::successors(Some(me), |l| l.fallback()).map(|l| l.tag().to_owned()).collect();
        let files: Vec<&str> = std::iter::successors(Some(me), |l| l.fallback()).map(|l| APP.get().and_then(|a| a.iter().find(|c| c.0 == l)).map(|c| c.1).or_else(|| FILES.iter().find(|f| f.0 == l).map(|f| f.1)).unwrap_or("?")).collect();
        Err(I18nError::MissingKey { key: key.to_owned(), chain: chain.join(" → "), files: files.join(", ") })
    }
    /// A number the locale's way: grouped thousands, `decimals` places.
    pub fn number(&self, v: f64, decimals: usize) -> String {
        let s = format!("{:.*}", decimals, v.abs());
        let (int, frac) = s.split_once('.').map_or((s.as_str(), ""), |(a, b)| (a, b));
        let mut g = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 { g.push(','); }
            g.push(c);
        }
        let sign = if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') { "-" } else { "" };
        if frac.is_empty() { format!("{sign}{g}") } else { format!("{sign}{g}.{frac}") }
    }
    /// A calendar date the locale's way.
    /// Other locales give `locale.date` in their catalog (`{year}`, `{month}`, `{day}`), else ISO 8601.
    pub fn date(&self, year: i32, month: u32, day: u32) -> String {
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        match self {
            Locale::ZhHans => format!("{year}年{month}月{day}日"),
            Locale::En => format!("{} {day}, {year}", MONTHS[(month.clamp(1, 12) - 1) as usize]),
            Locale::Other(_) => {
                let args = [("year", Arg::Str(year.to_string())), ("month", Arg::Str(month.to_string())), ("day", Arg::Str(day.to_string()))];
                lookup(*self, "locale.date", &args).map(|f| fill(*self, &f, &args)).unwrap_or_else(|| format!("{year:04}-{month:02}-{day:02}"))
            }
        }
    }
}

/// An argument of a message: text, or a number (written the locale's way, and it chooses plural forms).
#[derive(Clone, Debug, PartialEq)]
pub enum Arg { Str(String), Num(f64) }
impl From<&str> for Arg { fn from(s: &str) -> Self { Arg::Str(s.to_owned()) } }
impl From<String> for Arg { fn from(s: String) -> Self { Arg::Str(s) } }
impl From<&String> for Arg { fn from(s: &String) -> Self { Arg::Str(s.clone()) } }
impl From<f64> for Arg { fn from(v: f64) -> Self { Arg::Num(v) } }
impl From<f32> for Arg { fn from(v: f32) -> Self { Arg::Num(v as f64) } }
impl From<i32> for Arg { fn from(v: i32) -> Self { Arg::Num(v as f64) } }
impl From<i64> for Arg { fn from(v: i64) -> Self { Arg::Num(v as f64) } }
impl From<u32> for Arg { fn from(v: u32) -> Self { Arg::Num(v as f64) } }
impl From<usize> for Arg { fn from(v: usize) -> Self { Arg::Num(v as f64) } }

/// `t!(locale, "key")` or `t!(locale, "key", name = value, …)`: the text for `key` in `locale`.
#[macro_export]
macro_rules! t {
    ($loc:expr, $key:expr) => { $loc.tr($key) };
    ($loc:expr, $key:expr, $($name:ident = $val:expr),+ $(,)?) => { $loc.tr_args($key, &[$((stringify!($name), $crate::i18n::Arg::from($val))),+]) };
}

thread_local! {
    static CURRENT: Cell<Locale> = const { Cell::new(Locale::ZhHans) };
    static MISSES: RefCell<std::collections::BTreeSet<String>> = const { RefCell::new(std::collections::BTreeSet::new()) };
}
/// The locale views are being built in (the runtime sets it from the state before each view).
pub fn current() -> Locale { CURRENT.with(|c| c.get()) }
pub fn set_current(l: Locale) { CURRENT.with(|c| c.set(l)); }
/// The text for `key` in the current locale (what widgets use for their own strings).
pub fn tr(key: &str) -> String { current().tr(key) }
pub fn tr_args(key: &str, args: &[(&str, Arg)]) -> String { current().tr_args(key, args) }
/// Keys asked for and found in no catalog since the last call (and forget them).
pub fn misses() -> Vec<String> { MISSES.with(|m| std::mem::take(&mut *m.borrow_mut()).into_iter().collect()) }

/// One catalog: key → text (plural forms under `key.category`).
pub type Catalog = HashMap<String, String>;
/// Parse a catalog file (see the module's documentation).
pub fn parse(src: &str) -> Result<Catalog, I18nError> {
    let mut m = Catalog::new();
    let mut last: Option<String> = None;
    for (n, line) in src.lines().enumerate() {
        if line.starts_with("  ") && let Some(k) = &last {
            let v = m.get_mut(k).expect("the key just added");
            v.push('\n');
            v.push_str(line.trim_start());
            continue;
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { last = None; continue; }
        let Some((k, v)) = t.split_once('=') else { return Err(I18nError::Syntax { line: n + 1, text: t.to_owned() }) };
        let k = k.trim().to_owned();
        if m.insert(k.clone(), v.trim().to_owned()).is_some() { return Err(I18nError::Duplicate { line: n + 1, key: k }); }
        last = Some(k);
    }
    Ok(m)
}
/// An application's catalog files: (locale, path, text).
pub type Files = &'static [(Locale, &'static str, &'static str)];
static APP: OnceLock<Vec<(Locale, &'static str, Catalog)>> = OnceLock::new();
/// Install an application's catalogs (one per locale; see the module's documentation), looked up before
/// the framework's. The first call wins; later calls with the same files change nothing, and with other
/// files they are an error.
pub fn install(files: Files) -> Result<(), I18nError> {
    let parsed = files.iter().map(|(l, path, src)| parse(src).map(|c| (*l, *path, c)).map_err(|e| I18nError::Catalog { path: (*path).to_owned(), error: e.to_string() })).collect::<Result<Vec<_>, _>>()?;
    match APP.get() {
        Some(have) if *have == parsed => Ok(()),
        Some(_) => Err(I18nError::AlreadyInstalled),
        None => { let _ = APP.set(parsed); Ok(()) }
    }
}
/// The installed application catalog for `l`, if any.
pub fn app_catalog(l: Locale) -> Option<&'static Catalog> { APP.get().and_then(|a| a.iter().find(|c| c.0 == l)).map(|c| &c.2) }
/// The framework's catalog for `l`, parsed once (empty for a locale it ships none for).
pub fn catalog(l: Locale) -> &'static Catalog {
    static EMPTY: OnceLock<Catalog> = OnceLock::new();
    static ALL: OnceLock<Vec<(Locale, Catalog)>> = OnceLock::new();
    let all = ALL.get_or_init(|| FILES.iter().map(|(l, path, src)| (*l, parse(src).unwrap_or_else(|e| panic!("{path}: {e}")))).collect());
    all.iter().find(|c| c.0 == l).map_or_else(|| EMPTY.get_or_init(Catalog::new), |c| &c.1)
}
/// The framework's catalog files (locale, path, text), for tools and tests.
pub fn files() -> &'static [(Locale, &'static str, &'static str)] { &FILES }
/// The locales with an installed application catalog, in the order given.
pub fn installed() -> Vec<Locale> { APP.get().map(|a| a.iter().map(|c| c.0).collect()).unwrap_or_default() }

fn lookup(l: Locale, key: &str, args: &[(&str, Arg)]) -> Option<String> {
    app_catalog(l).and_then(|c| find(c, l, key, args)).or_else(|| find(catalog(l), l, key, args))
}
fn find(c: &Catalog, l: Locale, key: &str, args: &[(&str, Arg)]) -> Option<String> {
    if let Some(v) = c.get(key) { return Some(v.clone()); }
    // A plural message: the form for the count.
    let n = args.iter().find(|a| a.0 == "n").or_else(|| args.iter().find(|a| matches!(a.1, Arg::Num(_)))).and_then(|a| match a.1 { Arg::Num(v) => Some(v), _ => None });
    let n = n?;
    c.get(&format!("{key}.{}", l.plural(n))).or_else(|| c.get(&format!("{key}.other"))).cloned()
}
fn fill(l: Locale, s: &str, args: &[(&str, Arg)]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let r = &rest[i..];
        if let Some(t) = r.strip_prefix("{{") { out.push('{'); rest = t; continue; }
        if let Some(t) = r.strip_prefix("}}") { out.push('}'); rest = t; continue; }
        if r.starts_with('{') && let Some(j) = r.find('}') {
            let name = &r[1..j];
            match args.iter().find(|a| a.0 == name) {
                Some((_, Arg::Str(v))) => out.push_str(v),
                Some((_, Arg::Num(v))) => out.push_str(&if v.fract() == 0.0 { l.number(*v, 0) } else { l.number(*v, 2).trim_end_matches('0').trim_end_matches('.').to_owned() }),
                None => { out.push_str(&r[..=j]); }
            }
            rest = &r[j + 1..];
            continue;
        }
        out.push_str(&r[..1]);
        rest = &r[1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_fills_and_chooses_plurals() {
        let c = parse("# c\na = Hello {name}\nb.one = {n} file\nb.other = {n} files\nlong = one\n  two\n").expect("parses");
        assert_eq!(c["a"], "Hello {name}");
        assert_eq!(c["long"], "one\ntwo");
        assert_eq!(fill(Locale::En, "Hello {name}, {{x}}", &[("name", Arg::from("you"))]), "Hello you, {x}");
        assert_eq!(fill(Locale::En, "{n} rows", &[("n", Arg::from(12345))]), "12,345 rows");
        assert_eq!(Locale::En.plural(1.0), "one");
        assert_eq!(Locale::En.plural(2.0), "other");
        assert_eq!(Locale::ZhHans.plural(1.0), "other");
        assert!(parse("x").is_err());
        assert!(parse("a = 1\na = 2").is_err());
    }
    #[test]
    fn numbers_dates_and_the_fallback_chain() {
        assert_eq!(Locale::En.number(1234567.891, 2), "1,234,567.89");
        assert_eq!(Locale::ZhHans.number(-999.0, 0), "-999");
        assert_eq!(Locale::ZhHans.date(2026, 11, 15), "2026年11月15日");
        assert_eq!(Locale::En.date(2026, 11, 15), "Nov 15, 2026");
        assert_eq!(Locale::from_tag("zh-CN"), Some(Locale::ZhHans));
        assert_eq!(Locale::from_tag("en-GB"), Some(Locale::En));
        let e = Locale::ZhHans.try_tr("no.such.key", &[]).expect_err("missing");
        assert!(e.to_string().contains("no.such.key") && e.to_string().contains("zh-Hans → en"), "{e}");
        assert_eq!(Locale::En.tr("no.such.key"), "⟨no.such.key⟩");
        assert!(misses().contains(&"no.such.key".to_owned()));
        assert_eq!(t!(Locale::En, "widget.dialog.close"), "Close");
        assert_eq!(t!(Locale::En, "widget.list.rows", n = 1), "1 row");
        assert_eq!(t!(Locale::En, "widget.list.rows", n = 10000), "10,000 rows");
    }
    #[test]
    fn an_application_catalog_comes_first_and_falls_back_to_the_framework() {
        static FILES: [(Locale, &str, &str); 2] = [(Locale::ZhHans, "app/zh-Hans.txt", "app.title = 标题\n"), (Locale::En, "app/en.txt", "app.title = Title\n")];
        install(&FILES).expect("installs");
        install(&FILES).expect("the same files again change nothing");
        assert_eq!(install(&FILES[..1]), Err(I18nError::AlreadyInstalled));
        assert_eq!(t!(Locale::ZhHans, "app.title"), "标题");
        assert_eq!(t!(Locale::En, "widget.image.alt"), "Picture");
        assert_eq!(t!(Locale::ZhHans, "widget.image.alt"), "图片");
        let e = Locale::En.try_tr("app.nothing", &[]).expect_err("missing");
        assert!(e.to_string().contains("app/en.txt"), "{e}");
    }
}
