//! SW-12 — the machine-readable cenovnik, rendered.
//!
//! Pure: no database, no clock, no filesystem. Rows in, file out — so what the
//! shop publishes under ZZP čl. 6 st. 2 is decided by one function that can be
//! read end to end, and the archive's `content_hash` is a function of the
//! catalog rather than of whatever order a SELECT happened to return.
//!
//! Legal authority: `docs/REMAINING-SW-VERIFIED-RULES.md` §2b, §4 reqs. 10, 16.
//! Design: `docs/superpowers/specs/2026-08-01-sw12-cenovnik-design.md`.
//!
//! **No format is legally mandated** — the čl. 6 st. 7 bylaw does not exist, so
//! the shape below is the de facto data.gov.rs practice (req. 16), not law, and
//! swapping it is a routine release rather than a compliance event.
//!
//! The API is consumed by the publish path and the archive (Tasks 3, 4 and 7),
//! so `dead_code` is allowed here while it lands ahead of them — mirroring the
//! other domain modules (`kep`, `reklamacije`, `campaign_evidence`).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app_error::AppError;

/// The published columns, in order (req. 16).
///
/// ASCII on purpose: these are machine field names in a file meant to be parsed,
/// not operator-facing prose, and the de facto shape spells them without
/// diacritics. The *data* carries Serbian Latin verbatim — which is what the BOM
/// below is for.
pub const COLUMNS: [&str; 8] = [
    "sifra",
    "barkod",
    "naziv",
    "jedinica_mere",
    "prodajna_cena",
    "jedinicna_cena",
    "jedinica_za_jedinicnu_cenu",
    "datum_azuriranja",
];

const SEPARATOR: char = ';';

/// CRLF, per RFC 4180. `str::lines` reads either, so nothing in-tree depends on
/// the choice; a strict third-party parser might.
const LINE_ENDING: &str = "\r\n";

/// One catalog row as the published file sees it.
///
/// The unit-price pair is carried in its stored shape — the measure plus the
/// content of one selling unit on the schema-wide milli scale (v19) — rather
/// than pre-divided by the caller, so the division and its empty cases live in
/// one tested place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CenovnikRow {
    pub sifra: String,
    pub barkod: Option<String>,
    pub naziv: String,
    /// The measure the goods are SOLD in (`products.unit_of_measure`).
    pub jedinica_mere: String,
    pub prodajna_cena_minor: i64,
    /// The measure the jedinična cena is EXPRESSED in
    /// (`products.jedinicna_cena_jedinica`). `None` means the shop has not said,
    /// and nothing here will guess one.
    pub jedinicna_cena_jedinica: Option<String>,
    /// The content of one selling unit in that measure, value × 1000
    /// (`products.jedinicna_cena_sadrzaj_milli`): a 0,75 l bottle is `750`.
    pub jedinicna_cena_sadrzaj_milli: Option<i64>,
    /// RFC3339, as every timestamp in this crate travels.
    pub datum_azuriranja: String,
}

impl CenovnikRow {
    /// The jedinična cena in para per one unit of the measure, or `None` when the
    /// row cannot state one.
    ///
    /// Čl. 6 st. 2's second sentence pulls st. 1 into the published file, so this
    /// is not decoration. But an unconfigured product yields `None` and publishes
    /// an empty cell: a jedinična cena inferred from a package size nobody
    /// entered would be published as fact, and the shop is answerable under
    /// čl. 6 st. 4 for what it publishes. A missing cell is a visible gap; a
    /// guessed one is not.
    pub fn jedinicna_cena_minor(&self) -> Option<i64> {
        self.jedinicna_cena_jedinica.as_ref()?;
        let Some(sadrzaj_milli) = self.jedinicna_cena_sadrzaj_milli else {
            // One selling unit IS one of the measure (v19), so the two coincide.
            return Some(self.prodajna_cena_minor);
        };
        if sadrzaj_milli <= 0 {
            // The v19 CHECK refuses this; a row that reached us anyway divides by
            // nothing, and no cell beats a wrong one.
            return None;
        }
        // i128 so an absurd price cannot overflow the ×1000 and wrap into a
        // plausible-looking unit price; rounded to the nearest para, because
        // truncation publishes 6,66 where the true figure is 6,67.
        let scaled = i128::from(self.prodajna_cena_minor) * 1000;
        let rounded = (scaled + i128::from(sadrzaj_milli) / 2) / i128::from(sadrzaj_milli);
        i64::try_from(rounded).ok()
    }
}

/// Why a product's unit-price configuration cannot state a jedinična cena the
/// shop could stand behind.
///
/// Each variant carries the operator-facing wording, so every writer that
/// refuses the state refuses it in the same words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JedinicnaCenaDefect {
    /// A sadržaj of zero or less divides by nothing.
    SadrzajNotPositive,
    /// A sadržaj with no jedinica names no measure to divide into.
    SadrzajWithoutJedinica,
    /// A jedinica that differs from the measure the goods are SOLD in, with no
    /// package content to bridge them.
    DifferingJedinicaWithoutSadrzaj,
}

impl JedinicnaCenaDefect {
    /// The Serbian operator message.
    pub fn message(self) -> &'static str {
        match self {
            Self::SadrzajNotPositive => "Sadržaj pakovanja mora biti veći od nule.",
            Self::SadrzajWithoutJedinica => {
                "Uz sadržaj pakovanja izaberite i jedinicu za jediničnu cenu."
            }
            Self::DifferingJedinicaWithoutSadrzaj => {
                "Jedinica za jediničnu cenu se razlikuje od jedinice mere — unesite sadržaj pakovanja. Prazan sadržaj znači da je jedna prodajna jedinica jednaka jednoj jedinici mere (na primer 1 kom = 1 l)."
            }
        }
    }

    /// The form field the message points at, in the frontend's camelCase.
    pub fn field(self) -> &'static str {
        match self {
            Self::SadrzajNotPositive | Self::DifferingJedinicaWithoutSadrzaj => {
                "jedinicnaCenaSadrzajMilli"
            }
            Self::SadrzajWithoutJedinica => "jedinicnaCenaJedinica",
        }
    }
}

/// The one rule every writer of `products` applies before storing the unit-price
/// triple — the catalog form (`commands::catalog`) and the CSV import
/// (`importer`) both call this rather than keeping a copy.
///
/// That single home is the point. The state this refuses reached the published
/// file precisely because there were two write paths and only one guard: the
/// form learned the rule, the importer did not, and an import could leave a
/// product in exactly the shape the form had just started refusing.
///
/// What it enforces, given the measure the goods are SOLD in
/// (`products.unit_of_measure`) and the stored jedinična cena pair:
///
/// * a sadržaj must be positive — the v19 CHECK says so, and a non-positive one
///   divides by nothing;
/// * a sadržaj must name its jedinica — otherwise it can state no jedinična
///   cena at all (ZZP čl. 6 st. 1);
/// * a jedinica with **no** sadržaj is the v19 convention for „jedna prodajna
///   jedinica JESTE jedna jedinica mere“, and [`CenovnikRow::jedinicna_cena_minor`]
///   accordingly publishes the sale price AS the jedinična cena. That is right
///   exactly while the two measures agree, and provably wrong the moment they
///   differ: a 0,75 l bottle sold „po komadu“ at 279,00 would publish 279,00
///   per litre where the figure is 372,00, and čl. 6 st. 4 makes the shop
///   answer for the published number.
///
/// The measures are compared case-insensitively because „L“ and „l“ are one
/// measure to a shopper, and a shop that really sells one litre per piece still
/// has a way to say so: a sadržaj of 1.
pub fn validate_jedinicna_cena(
    unit_of_measure: &str,
    jedinicna_cena_jedinica: Option<&str>,
    jedinicna_cena_sadrzaj_milli: Option<i64>,
) -> Result<(), JedinicnaCenaDefect> {
    if let Some(sadrzaj_milli) = jedinicna_cena_sadrzaj_milli {
        if sadrzaj_milli <= 0 {
            return Err(JedinicnaCenaDefect::SadrzajNotPositive);
        }

        if jedinicna_cena_jedinica.is_none() {
            return Err(JedinicnaCenaDefect::SadrzajWithoutJedinica);
        }
    }

    if let Some(jedinica) = jedinicna_cena_jedinica {
        if jedinicna_cena_sadrzaj_milli.is_none()
            && jedinica.to_lowercase() != unit_of_measure.to_lowercase()
        {
            return Err(JedinicnaCenaDefect::DifferingJedinicaWithoutSadrzaj);
        }
    }

    Ok(())
}

/// Renders the published file: BOM, header, one line per row, CRLF-terminated.
///
/// Rows are emitted in `sifra` order regardless of the order given (ties keep
/// their input order), so the same catalog always renders the same bytes and
/// therefore the same [`content_hash`]. Without that, a caller changing its
/// ORDER BY would look like a price change to the archive.
pub fn render_csv(rows: &[CenovnikRow]) -> String {
    let mut ordered: Vec<&CenovnikRow> = rows.iter().collect();
    ordered.sort_by(|left, right| left.sifra.cmp(&right.sifra));

    // The BOM is not cosmetic: without it a spreadsheet opening the file under a
    // legacy codepage renders „Košulja“ as mojibake, and the naziv is what a
    // consumer matches the goods on.
    let mut out = String::from('\u{feff}');
    out.push_str(&line(&COLUMNS.map(|column| column.to_string())));

    for row in ordered {
        let jedinicna_cena = row
            .jedinicna_cena_minor()
            .map(format_para_2dec)
            .unwrap_or_default();
        out.push_str(&line(&[
            escape(&row.sifra),
            // Quoted as text so a consumer honouring CSV quoting keeps a 13-digit
            // barcode a string: read as a number, `0123456789012` loses its
            // leading zero and stops matching the goods.
            row.barkod.as_deref().map(as_text).unwrap_or_default(),
            escape(&row.naziv),
            escape(&row.jedinica_mere),
            format_para_2dec(row.prodajna_cena_minor),
            jedinicna_cena,
            row.jedinicna_cena_jedinica
                .as_deref()
                .map(escape)
                .unwrap_or_default(),
            format_date(&row.datum_azuriranja),
        ]));
    }

    out
}

/// The prodajna cena the file publishes for each šifra, in integer para.
///
/// The exact inverse of [`render_csv`], and the till guard's only source for
/// „the last published price“ (req. 12): čl. 6 st. 4 binds the shop to the
/// prices in the file it published, so the comparison has to be made against
/// that file rather than against the catalog the file was rendered from — which
/// is the very thing that may have moved without a republish.
///
/// Columns are located by **name** out of the file's own header, not by
/// position. The guard reads whichever snapshot is currently published, and an
/// upgrade that reorders [`COLUMNS`] leaves the outlet's current file carrying
/// the old header until the next price write.
///
/// Lenient by construction: a file this cannot read yields no prices, and a row
/// whose price cell will not parse is skipped rather than guessed at. No prices
/// means no guard — the same answer as an outlet that has published nothing —
/// because a till that warned against a figure nobody published would be worse
/// than one that stayed quiet.
pub fn published_prices(body: &str) -> BTreeMap<String, i64> {
    let mut lines = body.strip_prefix('\u{feff}').unwrap_or(body).lines();
    let Some(header) = lines.next().map(split_fields) else {
        return BTreeMap::new();
    };
    let position_of = |name: &str| header.iter().position(|column| column == name);
    let (Some(sifra_at), Some(cena_at)) = (position_of("sifra"), position_of("prodajna_cena"))
    else {
        return BTreeMap::new();
    };

    let mut prices = BTreeMap::new();
    for line in lines {
        let fields = split_fields(line);
        let (Some(sifra), Some(cena)) = (fields.get(sifra_at), fields.get(cena_at)) else {
            continue;
        };
        if sifra.is_empty() {
            continue;
        }
        if let Some(minor) = parse_para_2dec(cena) {
            prices.insert(sifra.clone(), minor);
        }
    }
    prices
}

/// SHA-256 of the rendered body, lowercase hex — the archive's handle on „which
/// file was published“ (čl. 6 st. 5) and the only cheap way to tell a republish
/// that changed something from one that changed nothing.
pub fn content_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());

    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// What a publish target did with a rendered cenovnik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishOutcome {
    /// The target accepted the body. The string names where it went and is what
    /// `cenovnik_snapshots.published_target` records — a snapshot that claims a
    /// publication has to say which one.
    Published { target: String },
    /// Nothing is configured to receive the file (req. 15 — a founder decision,
    /// not an engineering one). NOT an error: the snapshot is still generated
    /// and archived, and a price save must never fail because the hosting
    /// question is open. A generated-but-unpublished snapshot is an honest state
    /// the v19 schema models with a NULL `published_at`.
    NotConfigured,
}

/// Where a rendered cenovnik goes once it exists.
///
/// **The publish path calls this AFTER the price write has committed** — a
/// target must never be handed prices the catalog then rolls back, because
/// čl. 6 st. 4 binds the shop to what it published.
///
/// # The fetchability contract (req. 13)
///
/// Čl. 6 st. 5 is a duty to **enable** a comparison of the previously published
/// prices with the ones published in real time. A target that makes the file
/// hard to fetch therefore manufactures a breach for the very shop it was built
/// to protect — the file exists, and the comparison the statute asks for still
/// cannot be made. **Every implementation of this trait owes all seven, the
/// local folder below and the hosted endpoint req. 15 leaves to the founder
/// alike:**
///
/// 1. **No login and no session.** Whoever compares prices is a consumer or an
///    inspector, and neither has an account with the shop.
/// 2. **No CAPTCHA, no bot-fight rule and no WAF challenge.** The reader that
///    matters most is a script comparing many traders at once, which is exactly
///    what a bot challenge is built to stop.
/// 3. **No JavaScript-rendering requirement.** A plain file at a plain URL: a
///    page that assembles the prices in a browser publishes nothing to anything
///    that does not run one.
/// 4. **No `robots.txt` disallow** covering the file's path.
/// 5. **No aggressive rate limit.** A comparison across a shop's whole catalog
///    is one fetch; a limit that answers the second one with 429 defeats it.
/// 6. **A stable URL per prodajni objekat.** The address may not move between
///    republishes — see [`published_file_name`], which is why the name is
///    derived from the outlet and never from the date.
/// 7. **The correct `Content-Type`** for what was rendered: `text/csv` with
///    `charset=utf-8` for this file. A cenovnik served as
///    `application/octet-stream` is a download prompt, not a published price.
///
/// None of this is testable against an endpoint that does not exist yet, so
/// `the_fetchability_contract_is_written_into_the_publish_target_trait` asserts
/// the contract itself is still here for whoever writes that endpoint.
pub trait PublishTarget {
    fn publish(
        &self,
        body: &str,
        prodajno_mesto: &str,
        now: &str,
    ) -> Result<PublishOutcome, AppError>;
}

/// The default target: none. Nothing leaves the machine, and nothing fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotConfigured;

impl PublishTarget for NotConfigured {
    fn publish(
        &self,
        _body: &str,
        _prodajno_mesto: &str,
        _now: &str,
    ) -> Result<PublishOutcome, AppError> {
        Ok(PublishOutcome::NotConfigured)
    }
}

/// The local half of req. 15: the file lands in a folder on this machine.
///
/// The folder is the shop's own — a folder its web host serves, a folder a sync
/// client mirrors, or simply the folder it uploads from by hand. **Nothing here
/// touches the network.** Whether Actaer runs a public endpoint is a founder
/// decision (req. 15) and this cycle does not make it; what it does make is a
/// shop with a website able to discharge čl. 6 st. 2 today, by pointing this at
/// the folder that site publishes.
///
/// The trait's fetchability contract is satisfied trivially here: a file on
/// disk has no login, no challenge and no rate limit, its name is stable per
/// outlet, and the `.csv` extension is what makes an ordinary web server answer
/// with `text/csv`.
#[derive(Debug, Clone)]
pub struct LocalFolderTarget {
    folder: PathBuf,
}

impl LocalFolderTarget {
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        Self {
            folder: folder.into(),
        }
    }

    /// Where this target puts `prodajno_mesto`'s cenovnik.
    pub fn path_for(&self, prodajno_mesto: &str) -> PathBuf {
        self.folder.join(published_file_name(prodajno_mesto))
    }
}

impl PublishTarget for LocalFolderTarget {
    fn publish(
        &self,
        body: &str,
        prodajno_mesto: &str,
        _now: &str,
    ) -> Result<PublishOutcome, AppError> {
        // The operator names the folder in settings; nothing guarantees it
        // exists. Refusing until they create it by hand would leave the shop
        // with a configured target and no published file.
        std::fs::create_dir_all(&self.folder)?;

        let path = self.path_for(prodajno_mesto);
        // Written beside its destination and renamed over it: a rename within
        // one directory is atomic, so a fetch that lands mid-publish reads the
        // previous cenovnik rather than half of the new one — and čl. 6 st. 4
        // binds the shop to whatever that fetch returns.
        let temporary = self.folder.join(staging_file_name(prodajno_mesto));
        let written = std::fs::write(&temporary, body.as_bytes())
            .and_then(|()| std::fs::rename(&temporary, &path));
        if let Err(error) = written {
            // A half-written temporary in a folder the shop serves would be
            // served too, and nothing else would ever clean it up.
            let _ = std::fs::remove_file(&temporary);
            return Err(error.into());
        }

        Ok(PublishOutcome::Published {
            target: path.display().to_string(),
        })
    }
}

/// The name a publish stages an outlet's body under before renaming it over
/// [`published_file_name`].
///
/// **Unique per call**, which is the whole point: the published name is
/// deterministic per outlet (req. 13, rule 6), so a staged name derived from it
/// alone would be one path that every concurrent publish of that outlet wrote
/// to — two windows, an import racing a price edit, two installs syncing one
/// folder. The second write would then tear the body the first is about to
/// rename into place, publishing exactly the half-written cenovnik the staging
/// step exists to prevent. Process id, nanoseconds and a per-process counter,
/// so it is unique between processes and within one.
///
/// A sibling of the published file, because a rename is only atomic within one
/// directory. Dot-prefixed and `.tmp`-suffixed so a folder the shop serves does
/// not offer a partially written file as a cenovnik in the window before the
/// rename.
fn staging_file_name(prodajno_mesto: &str) -> String {
    static STAGED: AtomicU64 = AtomicU64::new(0);

    // Never `expect`: this runs after the price write has committed, on a path
    // whose contract is that it cannot panic on the sale that produced it.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let sequence = STAGED.fetch_add(1, Ordering::Relaxed);

    format!(
        ".{}.{}-{nanos}-{sequence}.tmp",
        published_file_name(prodajno_mesto),
        std::process::id()
    )
}

/// The file name an outlet's cenovnik is published under.
///
/// **Stable per prodajni objekat** (req. 13, rule 6): derived from the frozen
/// archive key and never from the date, so the address a consumer bookmarked or
/// an inspector was given keeps resolving to the file in force. A dated name
/// would hand every republish a new URL and break the čl. 6 st. 5 comparison at
/// the moment it is asked for.
///
/// **Folded to ASCII**, because this segment is also the last segment of the
/// eventual URL: a name carrying č or đ is normalised differently by different
/// filesystems and re-escaped differently by each link in the chain, which is
/// how a „stable URL“ quietly stops being stable. The `.csv` extension is what
/// makes an ordinary web server answer with `text/csv` (rule 7) without further
/// configuration.
///
/// And it is why free text can never compose a path: `/`, `\` and `.` are not
/// in the retained set, so `../../etc/passwd` folds to one flat segment inside
/// the configured folder.
pub fn published_file_name(prodajno_mesto: &str) -> String {
    format!("cenovnik-{}.csv", slug(prodajno_mesto))
}

/// Long enough for a Serbian street address, short enough to stay inside the
/// path limits of every filesystem the app runs on once the folder is prepended.
const SLUG_MAX: usize = 60;

/// What an outlet whose whole name folds away is published as. It still has to
/// publish somewhere, and this has to be as stable as any other name.
const FALLBACK_SLUG: &str = "objekat";

fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut separator_pending = false;

    for character in value.chars().map(fold_diacritic) {
        if character.is_ascii_alphanumeric() {
            if separator_pending && !out.is_empty() {
                out.push('-');
            }
            separator_pending = false;
            out.push(character.to_ascii_lowercase());
            if out.len() >= SLUG_MAX {
                break;
            }
        } else {
            separator_pending = true;
        }
    }

    if out.is_empty() {
        FALLBACK_SLUG.to_string()
    } else {
        out
    }
}

/// Serbian Latin to its ASCII skeleton. Anything else is left alone and falls
/// out as a separator above — including Cyrillic, which has no one-to-one Latin
/// fold this function could honestly claim to make.
fn fold_diacritic(character: char) -> char {
    match character {
        'č' | 'ć' => 'c',
        'Č' | 'Ć' => 'C',
        'ž' => 'z',
        'Ž' => 'Z',
        'š' => 's',
        'Š' => 'S',
        'đ' => 'd',
        'Đ' => 'D',
        other => other,
    }
}

fn line(fields: &[String]) -> String {
    let mut out = fields.join(&SEPARATOR.to_string());
    out.push_str(LINE_ENDING);
    out
}

/// RFC 4180 escaping against this file's separator.
fn escape(value: &str) -> String {
    if value.contains([SEPARATOR, '"', '\n', '\r']) {
        as_text(value)
    } else {
        value.to_string()
    }
}

fn as_text(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Integer para as `1234.56` — two decimals, a `.` decimal mark (req. 16), no
/// thousands grouping, never a float.
fn format_para_2dec(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// One rendered line back into its fields, unescaping RFC 4180 quoting.
///
/// A trailing CR is dropped here rather than at each call site: `str::lines`
/// splits on the LF of this file's CRLF and leaves the CR on the last field,
/// which would make every price cell unparseable.
fn split_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut characters = line.trim_end_matches('\r').chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' if quoted => {
                if characters.peek() == Some(&'"') {
                    current.push('"');
                    characters.next();
                } else {
                    quoted = false;
                }
            }
            '"' => quoted = true,
            SEPARATOR if !quoted => fields.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

/// The inverse of [`format_para_2dec`]: `1234.56` back into 123 456 para.
///
/// Integer arithmetic only, and deliberately strict — exactly two decimals, ASCII
/// digits, a `.` decimal mark. Anything else is not a price this crate rendered,
/// and a lenient parse would hand the till guard a number to compare against
/// that nobody published.
fn parse_para_2dec(value: &str) -> Option<i64> {
    let value = value.trim();
    let (sign, digits) = match value.strip_prefix('-') {
        Some(rest) => (-1_i64, rest),
        None => (1_i64, value),
    };
    let (whole, fraction) = digits.split_once('.')?;
    if whole.is_empty() || fraction.len() != 2 {
        return None;
    }
    if !whole
        .chars()
        .chain(fraction.chars())
        .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let fraction: i64 = fraction.parse().ok()?;
    whole
        .checked_mul(100)?
        .checked_add(fraction)
        .map(|minor| sign * minor)
}

/// `DD-MM-YYYY` (req. 16) from an RFC3339 stamp, in the stamp's own offset — the
/// same reading `kep.rs` takes of a booking date, and this crate stamps in UTC.
///
/// A stamp that will not parse is passed through verbatim rather than blanked:
/// the row's date is evidence of when the price last moved, and a visibly wrong
/// value is a defect somebody can see, where an empty cell is one nobody can.
fn format_date(rfc3339: &str) -> String {
    match OffsetDateTime::parse(rfc3339, &Rfc3339) {
        Ok(stamp) => format!(
            "{:02}-{:02}-{:04}",
            stamp.day(),
            stamp.month() as u8,
            stamp.year()
        ),
        Err(_) => escape(rfc3339.trim()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn row(sifra: &str) -> CenovnikRow {
        CenovnikRow {
            sifra: sifra.to_string(),
            barkod: None,
            naziv: "Košulja".to_string(),
            jedinica_mere: "kom".to_string(),
            prodajna_cena_minor: 279_900,
            jedinicna_cena_jedinica: None,
            jedinicna_cena_sadrzaj_milli: None,
            datum_azuriranja: "2026-08-02T09:15:00Z".to_string(),
        }
    }

    /// A 0,75 l bottle at 279,00 RSD: 750 milli of `l`, so 372,00 RSD per litar.
    fn row_with_unit_price() -> CenovnikRow {
        CenovnikRow {
            naziv: "Sok od jabuke 0,75 l".to_string(),
            prodajna_cena_minor: 27_900,
            jedinicna_cena_jedinica: Some("l".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(750),
            ..row("A-1")
        }
    }

    fn row_with_barcode(barkod: &str) -> CenovnikRow {
        CenovnikRow {
            barkod: Some(barkod.to_string()),
            ..row("A-1")
        }
    }

    fn row_priced(prodajna_cena_minor: i64) -> CenovnikRow {
        CenovnikRow {
            prodajna_cena_minor,
            ..row("A-1")
        }
    }

    fn data_row(csv: &str) -> String {
        csv.lines()
            .nth(1)
            .expect("one data row")
            .trim_end_matches('\r')
            .to_string()
    }

    fn data_fields(csv: &str) -> Vec<String> {
        data_row(csv).split(';').map(str::to_string).collect()
    }

    /// čl. 6 st. 2's second sentence pulls st. 1 into the published file, so a
    /// cenovnik carrying only prodajna cena does not discharge the duty.
    #[test]
    fn every_row_carries_the_unit_price_and_its_measure() {
        let csv = render_csv(&[row_with_unit_price()]);
        let header = csv.lines().next().expect("header");
        assert!(header.contains("jedinicna_cena"), "{header}");
        assert!(header.contains("jedinica_za_jedinicnu_cenu"), "{header}");
    }

    #[test]
    fn the_file_is_utf8_with_a_bom_and_semicolon_separated() {
        let csv = render_csv(&[row_with_unit_price()]);
        assert!(
            csv.starts_with('\u{feff}'),
            "a BOM keeps Excel from mangling the diacritics"
        );
        assert!(csv.lines().next().expect("header").contains(';'));
    }

    /// A 13-digit barcode read as a number loses its leading zero and stops
    /// matching the goods. Quote it as text.
    #[test]
    fn the_barcode_is_quoted_as_text() {
        let csv = render_csv(&[row_with_barcode("0123456789012")]);
        assert!(csv.contains("\"0123456789012\""), "{csv}");
    }

    #[test]
    fn prices_render_two_decimals_from_integer_para() {
        let csv = render_csv(&[row_priced(123_456)]); // 1234,56 RSD
        assert!(csv.contains("1234.56"), "{csv}");
        assert!(!csv.contains("1234.5600000"), "no float artefacts: {csv}");
    }

    #[test]
    fn the_hash_changes_when_any_price_changes() {
        let a = content_hash(&render_csv(&[row_priced(100)]));
        let b = content_hash(&render_csv(&[row_priced(101)]));
        assert_ne!(a, b);
    }

    /// The header is a contract with whoever parses the file, so its spelling and
    /// its order are pinned rather than left to whatever the constant happens to
    /// say today.
    #[test]
    fn the_header_is_the_published_column_order() {
        let csv = render_csv(&[]);
        assert_eq!(
            csv.lines().next().expect("header"),
            "\u{feff}sifra;barkod;naziv;jedinica_mere;prodajna_cena;\
             jedinicna_cena;jedinica_za_jedinicnu_cenu;datum_azuriranja"
        );
    }

    /// A shop with nothing to publish still publishes a well-formed file. The
    /// v19 archive allows `row_count = 0` for exactly this.
    #[test]
    fn an_empty_catalog_still_renders_the_header() {
        let csv = render_csv(&[]);
        assert!(csv.starts_with('\u{feff}'));
        assert_eq!(csv.lines().count(), 1, "header only: {csv:?}");
    }

    /// Req. 10's whole point: the unit price is derived from the content of one
    /// selling unit, not copied off the sale price. 279,00 RSD for 0,75 l is
    /// 372,00 RSD per litar.
    #[test]
    fn the_unit_price_is_derived_from_the_content_of_one_selling_unit() {
        let csv = render_csv(&[row_with_unit_price()]);
        let fields = data_fields(&csv);
        assert_eq!(fields[4], "279.00", "prodajna cena: {fields:?}");
        assert_eq!(fields[5], "372.00", "jedinična cena po litru: {fields:?}");
        assert_eq!(fields[6], "l", "{fields:?}");
    }

    /// A `sadržaj` of NULL means one selling unit is one of the measure, so the
    /// two prices legitimately coincide — the only case in which they may.
    #[test]
    fn a_unit_that_is_the_selling_unit_repeats_the_sale_price() {
        let row = CenovnikRow {
            jedinicna_cena_jedinica: Some("kom".to_string()),
            jedinicna_cena_sadrzaj_milli: None,
            ..row_priced(27_900)
        };
        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[5], "279.00", "{fields:?}");
        assert_eq!(fields[6], "kom", "{fields:?}");
    }

    /// Nothing here guesses. An unconfigured product publishes empty cells, so
    /// the gap is visible instead of being filled with a figure the shop would
    /// then be answerable for under čl. 6 st. 4.
    #[test]
    fn an_unconfigured_unit_price_leaves_the_cells_empty() {
        let unconfigured = row_priced(27_900);
        assert_eq!(unconfigured.jedinicna_cena_minor(), None);

        let fields = data_fields(&render_csv(&[unconfigured]));
        assert_eq!(fields[4], "279.00", "{fields:?}");
        assert_eq!(fields[5], "", "no guessed jedinična cena: {fields:?}");
        assert_eq!(fields[6], "", "{fields:?}");
    }

    /// Truncation would publish 6,66 where the true unit price is 6,67. Rounding
    /// is to the nearest para, and it is the published figure that must be right.
    #[test]
    fn the_unit_price_rounds_to_the_nearest_para() {
        let row = CenovnikRow {
            jedinicna_cena_jedinica: Some("kg".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(300),
            ..row_priced(200)
        };
        assert_eq!(row.jedinicna_cena_minor(), Some(667));
    }

    /// The v19 CHECK refuses a non-positive sadržaj; if one ever reaches the
    /// renderer it divides by nothing, and an empty cell beats a wrong price or a
    /// panic in the publish path.
    #[test]
    fn a_nonpositive_content_publishes_no_unit_price_instead_of_dividing() {
        let row = CenovnikRow {
            jedinicna_cena_jedinica: Some("kg".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(0),
            ..row_priced(27_900)
        };
        assert_eq!(row.jedinicna_cena_minor(), None);

        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[5], "", "{fields:?}");
    }

    /// The three states no writer of `products` may store, in the one place
    /// that decides them. Both writers — the catalog form and the CSV importer
    /// — refuse through this function, so neither can drift from the other.
    #[test]
    fn the_shared_validator_refuses_every_half_configured_pair() {
        assert_eq!(
            validate_jedinicna_cena("kom", Some("l"), Some(0)),
            Err(JedinicnaCenaDefect::SadrzajNotPositive)
        );
        assert_eq!(
            validate_jedinicna_cena("kom", None, Some(750)),
            Err(JedinicnaCenaDefect::SadrzajWithoutJedinica)
        );
        // The D2 state: a 0,75 l bottle sold „po komadu“ whose package content
        // nobody entered would publish its sale price as the price per litre.
        assert_eq!(
            validate_jedinicna_cena("kom", Some("l"), None),
            Err(JedinicnaCenaDefect::DifferingJedinicaWithoutSadrzaj)
        );
    }

    /// The v19 convention stays expressible, and the refusal above must not
    /// reach it: goods sold by the kilogram priced per kilogram need no package
    /// content, „L“ and „l“ are one measure to a shopper, and a described
    /// package divides correctly whatever the selling measure.
    #[test]
    fn the_shared_validator_accepts_every_configuration_that_divides() {
        assert_eq!(validate_jedinicna_cena("kg", Some("kg"), None), Ok(()));
        assert_eq!(validate_jedinicna_cena("L", Some("l"), None), Ok(()));
        assert_eq!(validate_jedinicna_cena("kom", Some("l"), Some(750)), Ok(()));
        // Nothing configured at all: the file publishes an empty cell, which is
        // a visible gap rather than a guess.
        assert_eq!(validate_jedinicna_cena("kom", None, None), Ok(()));
    }

    #[test]
    fn a_product_without_a_barcode_leaves_the_cell_empty() {
        let fields = data_fields(&render_csv(&[row_priced(100)]));
        assert_eq!(fields[1], "", "an absent barcode is empty, not \"\"");
    }

    /// The naziv is what a consumer matches the goods on, so a semicolon or a
    /// quote inside it must not shift every following column by one.
    #[test]
    fn a_naziv_carrying_the_separator_or_a_quote_is_escaped() {
        let row = CenovnikRow {
            naziv: "Košulja; „bela“ 15\" kragna".to_string(),
            ..row_priced(100)
        };
        let rendered = data_row(&render_csv(&[row]));
        assert!(
            rendered.contains("\"Košulja; „bela“ 15\"\" kragna\""),
            "{rendered}"
        );
        assert_eq!(
            rendered.matches("1.00").count(),
            1,
            "the columns must not shift: {rendered}"
        );
    }

    #[test]
    fn the_date_renders_dd_mm_yyyy() {
        let row = CenovnikRow {
            datum_azuriranja: "2026-08-02T23:15:00Z".to_string(),
            ..row_priced(100)
        };
        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[7], "02-08-2026", "{fields:?}");
    }

    /// A stamp the renderer cannot read is a defect upstream. Publishing it
    /// verbatim keeps that defect visible; blanking the cell hides it.
    #[test]
    fn an_unparseable_stamp_is_published_verbatim_rather_than_blanked() {
        let row = CenovnikRow {
            datum_azuriranja: "juče".to_string(),
            ..row_priced(100)
        };
        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[7], "juče", "{fields:?}");
    }

    /// The archive dedupes and compares on `content_hash`, so the same catalog
    /// must render the same bytes no matter what order it arrives in — otherwise
    /// a caller's ORDER BY looks like a price change (čl. 6 st. 5).
    #[test]
    fn the_render_is_ordered_by_sifra_so_the_hash_does_not_follow_the_select() {
        let first = row("A-1");
        let second = row("B-2");
        let third = row("C-3");

        let ascending = render_csv(&[first.clone(), second.clone(), third.clone()]);
        let shuffled = render_csv(&[third, first, second]);

        assert_eq!(ascending, shuffled);
        assert_eq!(content_hash(&ascending), content_hash(&shuffled));
        let sifre: Vec<&str> = ascending
            .lines()
            .skip(1)
            .map(|row| row.split(';').next().expect("sifra"))
            .collect();
        assert_eq!(sifre, ["A-1", "B-2", "C-3"]);
    }

    #[test]
    fn the_hash_is_sixty_four_hex_characters_and_stable_for_the_same_body() {
        let body = render_csv(&[row_with_unit_price()]);
        let hash = content_hash(&body);

        assert_eq!(hash.len(), 64, "{hash}");
        assert!(
            hash.chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_uppercase()),
            "{hash}"
        );
        assert_eq!(hash, content_hash(&body));
    }

    /// The BOM exists for this: the naziv reaches the file with its diacritics
    /// intact, and „ … “ are the Serbian quotes, not ASCII ones.
    #[test]
    fn serbian_latin_survives_the_render() {
        let row = CenovnikRow {
            naziv: "Čarape žute, Đorđević".to_string(),
            ..row_priced(100)
        };
        assert!(
            render_csv(&[row]).contains("Čarape žute, Đorđević"),
            "the diacritics must reach the file verbatim"
        );
    }

    /// The till guard (req. 12) compares against „the last published price“, and
    /// the only record of that is the published file itself. So the read-back has
    /// to be the exact inverse of the render — quoting, BOM, CRLF and all — or
    /// the guard measures the sale against a price the shop never published.
    #[test]
    fn the_published_prices_read_back_exactly_what_the_render_wrote() {
        let mut awkward = row_with_unit_price();
        awkward.sifra = "A;1\"x".to_string();
        let csv = render_csv(&[awkward, row("B-2"), row_with_barcode("0123456789012")]);

        let prices = published_prices(&csv);
        assert_eq!(prices.get("A;1\"x"), Some(&27_900), "{prices:?}");
        assert_eq!(prices.get("B-2"), Some(&279_900), "{prices:?}");
        assert_eq!(prices.get("A-1"), Some(&279_900), "{prices:?}");
        assert_eq!(prices.len(), 3, "{prices:?}");
    }

    /// Read back as integer para, never through a float: `1234.56` is 123 456
    /// para, and a parse that round-tripped through `f64` would put the guard's
    /// comparison one para off on figures a shop really charges.
    #[test]
    fn a_published_price_reads_back_as_integer_para() {
        let csv = render_csv(&[row_priced(123_456)]);
        assert_eq!(published_prices(&csv).get("A-1"), Some(&123_456));

        for rendered in ["1234.5", "1234.567", "1234", "besplatno", "", "12,34"] {
            assert_eq!(
                parse_para_2dec(rendered),
                None,
                "a cell that is not a two-decimal price is not a price: {rendered}"
            );
        }
        assert_eq!(parse_para_2dec("-1.05"), Some(-105));
        assert_eq!(parse_para_2dec("0.00"), Some(0));
    }

    /// The guard reads whichever file is currently published, and an upgrade that
    /// reorders [`COLUMNS`] leaves yesterday's snapshot carrying yesterday's
    /// header until the next price write. So the columns are located by name out
    /// of the file's own header rather than by position.
    #[test]
    fn the_price_column_is_located_by_the_files_own_header() {
        let reordered = "\u{feff}naziv;prodajna_cena;sifra\r\n\
                         Košulja;279.00;A-1\r\n";
        assert_eq!(published_prices(reordered).get("A-1"), Some(&27_900));
    }

    /// A file the guard cannot read is not a published price, and inventing one
    /// would have the till warn against a figure nobody published. No prices, no
    /// guard — the same answer as an outlet that has published nothing.
    #[test]
    fn a_body_without_the_two_columns_yields_no_comparable_price() {
        assert!(published_prices("").is_empty());
        assert!(published_prices("\u{feff}sifra;naziv\r\nA-1;Košulja\r\n").is_empty());
        assert!(published_prices("\u{feff}naziv;prodajna_cena\r\nKošulja;279.00\r\n").is_empty());
        // A row whose price cell will not parse is skipped; the rest still read.
        let partly = "\u{feff}sifra;prodajna_cena\r\nA-1;nema\r\nB-2;279.00\r\n";
        assert_eq!(
            published_prices(partly).into_iter().collect::<Vec<_>>(),
            [("B-2".to_string(), 27_900)]
        );
    }

    /// Spelled as the literal CRLF rather than through `LINE_ENDING` on purpose:
    /// a test that reads the constant it is guarding is a tautology and passes
    /// for any line ending at all.
    #[test]
    fn every_row_ends_with_crlf() {
        let csv = render_csv(&[row_priced(100)]);
        assert_eq!(csv.matches("\r\n").count(), 2, "{csv:?}");
        assert!(csv.ends_with("\r\n"), "{csv:?}");
        assert!(
            !csv.replace("\r\n", "").contains(['\r', '\n']),
            "no bare CR or LF may survive outside a CRLF pair: {csv:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Task 7 — publish targets (reqs. 13, 15)
    // ---------------------------------------------------------------------

    const OUTLET: &str = "Bulevar oslobođenja 1, Novi Sad";
    const OUTLET_FILE: &str = "cenovnik-bulevar-oslobodenja-1-novi-sad.csv";
    const NOW: &str = "2026-08-02T09:15:00Z";

    /// A folder of this test's own, removed afterwards. `std::env::temp_dir` and
    /// a nanosecond suffix, exactly as `db::test_database_path` does it — the
    /// crate carries no temp-file dependency.
    fn with_publish_folder(test_name: &str, test: impl FnOnce(&Path)) {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let folder = std::env::temp_dir().join(format!("vantumpos-{test_name}-{unique}"));
        std::fs::create_dir_all(&folder).expect("the publish folder should be created");

        test(&folder);

        std::fs::remove_dir_all(&folder).expect("the publish folder should be removed");
    }

    fn entries(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .expect("the publish folder should read")
            .map(|entry| {
                entry
                    .expect("a directory entry should read")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    /// Req. 15 is a founder decision, not an engineering one, so „nothing is
    /// configured“ has to be a state the caller can read — never an error. The
    /// publish runs after the price write has committed; an `Err` here would be
    /// logged as a failure on a save that in fact succeeded.
    #[test]
    fn nothing_configured_is_a_state_and_never_an_error() {
        let outcome = NotConfigured
            .publish(&render_csv(&[row_priced(27_900)]), OUTLET, NOW)
            .expect("a shop with no publish target has not failed at anything");
        assert_eq!(outcome, PublishOutcome::NotConfigured);
    }

    /// What the target writes has to be what the archive kept, byte for byte —
    /// BOM, CRLF and diacritics included. Čl. 6 st. 4 binds the shop to what a
    /// fetch of this file returns, so anything the adapter normalises on the way
    /// out is a price the shop is answerable for but never archived.
    #[test]
    fn the_local_folder_target_writes_the_snapshot_body_byte_for_byte() {
        with_publish_folder("cenovnik-local-target-body", |folder| {
            let body = render_csv(&[row_with_unit_price()]);
            let outcome = LocalFolderTarget::new(folder)
                .publish(&body, OUTLET, NOW)
                .expect("the local folder should accept the file");

            let path = folder.join(OUTLET_FILE);
            assert_eq!(
                outcome,
                PublishOutcome::Published {
                    target: path.display().to_string()
                },
                "the snapshot has to record which target took it"
            );

            let written = std::fs::read(&path).expect("the published file should read");
            assert_eq!(written.as_slice(), body.as_bytes(), "byte for byte");
            assert_eq!(
                &written[..3],
                [0xEF, 0xBB, 0xBF],
                "the BOM reaches the disk: {written:?}"
            );
            assert!(
                String::from_utf8(written)
                    .expect("the file is UTF-8")
                    .contains("Sok od jabuke 0,75 l"),
                "the diacritics survive the write"
            );
        });
    }

    /// Req. 13's stable URL per prodajni objekat. A dated file name would give
    /// every republish a new address and break the bookmark a consumer or an
    /// inspector was handed — at the very moment čl. 6 st. 5 asks them to
    /// compare the published prices with the realtime ones.
    #[test]
    fn republishing_overwrites_the_one_stable_file_per_outlet() {
        with_publish_folder("cenovnik-local-target-stable", |folder| {
            let target = LocalFolderTarget::new(folder);
            let first = target
                .publish(&render_csv(&[row_priced(27_900)]), OUTLET, NOW)
                .expect("first publish");
            let second = target
                .publish(
                    &render_csv(&[row_priced(31_900)]),
                    OUTLET,
                    "2026-08-03T09:15:00Z",
                )
                .expect("second publish");

            assert_eq!(first, second, "the address does not move between publishes");
            assert_eq!(entries(folder), [OUTLET_FILE], "one outlet, one file");
            let published =
                std::fs::read_to_string(folder.join(OUTLET_FILE)).expect("the file should read");
            assert!(published.contains("319.00"), "{published:?}");
            assert!(
                !published.contains("279.00"),
                "the fetched file is the current one: {published:?}"
            );
        });
    }

    /// Čl. 6 st. 2 publishes „posebno za svaki prodajni objekat“, so two outlets
    /// sharing one folder — a synced drive, a single web root — must not
    /// overwrite each other's cenovnik.
    #[test]
    fn two_outlets_publish_to_two_files() {
        with_publish_folder("cenovnik-local-target-per-outlet", |folder| {
            let target = LocalFolderTarget::new(folder);
            target
                .publish(&render_csv(&[row_priced(27_900)]), OUTLET, NOW)
                .expect("the first outlet publishes");
            target
                .publish(&render_csv(&[row_priced(31_900)]), "Kneza Miloša 8", NOW)
                .expect("the second outlet publishes");

            assert_eq!(
                entries(folder),
                [
                    "cenovnik-bulevar-oslobodenja-1-novi-sad.csv",
                    "cenovnik-kneza-milosa-8.csv"
                ]
            );
        });
    }

    /// The name is folded to ASCII because the last path segment is also the
    /// last segment of the eventual URL, and a name carrying č or đ is
    /// normalised differently by each filesystem and re-escaped differently by
    /// each link in the chain — which is how a „stable URL“ stops being stable.
    /// The `.csv` extension is what makes an ordinary web server answer with the
    /// right `Content-Type` without further configuration (req. 13).
    #[test]
    fn the_published_file_name_is_a_stable_ascii_name_ending_in_csv() {
        assert_eq!(published_file_name(OUTLET), OUTLET_FILE);
        assert_eq!(
            published_file_name("Čačak — Žitni trg 5/б"),
            "cenovnik-cacak-zitni-trg-5.csv"
        );
        assert_eq!(
            published_file_name("Đurđevdanska 12, Šabac"),
            "cenovnik-durdevdanska-12-sabac.csv"
        );

        // A name that folds away entirely still has to publish somewhere, and
        // the fallback has to be as stable as every other name.
        assert_eq!(published_file_name("   "), "cenovnik-objekat.csv");
        assert_eq!(published_file_name("«»"), "cenovnik-objekat.csv");

        // Bounded, and the same input always gives the same answer.
        let long = "Ulica ".repeat(60);
        assert!(published_file_name(&long).len() < 100, "{long}");
        assert_eq!(published_file_name(&long), published_file_name(&long));
    }

    /// `prodajno_mesto` is free text out of the shop's own settings. It names a
    /// file, so it must never be able to compose a path: a name carrying `..` or
    /// a separator has to fold into one flat segment inside the configured
    /// folder, not walk out of it.
    #[test]
    fn an_outlet_name_can_never_compose_a_path() {
        with_publish_folder("cenovnik-local-target-traversal", |folder| {
            let outcome = LocalFolderTarget::new(folder)
                .publish(&render_csv(&[row_priced(100)]), "../../etc/passwd", NOW)
                .expect("publish");

            assert_eq!(
                outcome,
                PublishOutcome::Published {
                    target: folder.join("cenovnik-etc-passwd.csv").display().to_string()
                }
            );
            assert_eq!(entries(folder), ["cenovnik-etc-passwd.csv"]);
        });
    }

    /// The staged body must not survive a publish that succeeded: a folder the
    /// shop points its web root at would serve the leftover too. That the body
    /// is staged at all is
    /// `a_failed_publish_leaves_the_previous_cenovnik_whole`'s job — this test
    /// alone is satisfied by any write that leaves one file behind.
    #[test]
    fn a_publish_leaves_no_temporary_file_behind() {
        with_publish_folder("cenovnik-local-target-no-temp", |folder| {
            LocalFolderTarget::new(folder)
                .publish(&render_csv(&[row_priced(100)]), OUTLET, NOW)
                .expect("publish");
            assert_eq!(entries(folder), [OUTLET_FILE]);
        });
    }

    /// Čl. 6 st. 4 binds the shop to whatever a fetch of the published file
    /// returns, so the served file is never opened for writing: the body is
    /// staged in a sibling temporary and renamed over the destination, and a
    /// publish that fails leaves the previous cenovnik whole rather than
    /// truncated down to half a body a consumer can hold the shop to.
    ///
    /// Unix-only, and for the same reason as backup's
    /// `perform_backup_records_failed_job_when_copy_fails`: a read-only folder
    /// is the one portable way to stop a NEW file being created while an
    /// existing file inside it stays writable — which is exactly the asymmetry
    /// that separates a rename from a write in place.
    #[cfg(unix)]
    #[test]
    fn a_failed_publish_leaves_the_previous_cenovnik_whole() {
        use std::os::unix::fs::PermissionsExt;

        with_publish_folder("cenovnik-local-target-atomic", |folder| {
            let target = LocalFolderTarget::new(folder);
            let published = render_csv(&[row_priced(27_900)]);
            target
                .publish(&published, OUTLET, NOW)
                .expect("the first publish should land");

            // The folder exists, so `create_dir_all` is a no-op success — but
            // nothing new can be created inside it, so the body cannot stage.
            // The published file itself stays writable: a publish that wrote
            // the destination in place would still succeed here.
            std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o500))
                .expect("read-only permissions should set");
            let result = target.publish(&render_csv(&[row_priced(31_900)]), OUTLET, NOW);
            // Restored before asserting, so the folder can be cleaned up.
            std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))
                .expect("permissions should restore");

            let error = result.expect_err("a folder that cannot stage the body cannot publish");
            assert_eq!(error.code(), "file_system_error");

            let served = std::fs::read(folder.join(OUTLET_FILE)).expect("the file should read");
            assert_eq!(
                served.as_slice(),
                published.as_bytes(),
                "a fetch during a failed publish reads the previous cenovnik, byte for byte"
            );
            assert_eq!(entries(folder), [OUTLET_FILE]);
        });
    }

    /// The name an outlet publishes under is deterministic, and it has to be
    /// (req. 13, rule 6). The name its body *stages* under must not be: two
    /// writers publishing one outlet at once — two windows, an import racing a
    /// price edit, two installs syncing one folder — would otherwise stage into
    /// a single path, and the second write would tear the body the first is
    /// about to rename into place. That is the exact failure the staging step
    /// exists to prevent.
    #[test]
    fn two_publishes_of_one_outlet_never_stage_into_the_same_file() {
        let first = staging_file_name(OUTLET);
        let second = staging_file_name(OUTLET);
        assert_ne!(first, second, "a staged body is per call, never per outlet");

        for staged in [&first, &second] {
            // Still a sibling of the published file — a rename is only atomic
            // within one directory — and still hidden from what a web root
            // would list as a cenovnik.
            assert!(!staged.contains(['/', '\\']), "{staged}");
            assert!(staged.starts_with(&format!(".{OUTLET_FILE}")), "{staged}");
            assert!(staged.ends_with(".tmp"), "{staged}");
        }
    }

    /// A publish can also fail after the body has staged, and the temporary is
    /// then a half-published cenovnik sitting in a folder the shop serves.
    /// Nothing else would ever clean it up, so the publish itself must.
    #[test]
    fn a_publish_that_fails_after_staging_removes_its_temporary() {
        with_publish_folder("cenovnik-local-target-temp-cleanup", |folder| {
            // A directory is the one thing a rename can never replace, so the
            // body stages and the rename over the destination is what fails.
            std::fs::create_dir(folder.join(OUTLET_FILE)).expect("the blocker should be created");

            let error = LocalFolderTarget::new(folder)
                .publish(&render_csv(&[row_priced(27_900)]), OUTLET, NOW)
                .expect_err("a destination that cannot be replaced cannot publish");
            assert_eq!(error.code(), "file_system_error");

            assert_eq!(
                entries(folder),
                [OUTLET_FILE],
                "the staged body must not be left in the folder the shop serves"
            );
        });
    }

    /// The operator names a folder in settings; nothing guarantees it exists
    /// yet, and refusing to publish until they create it by hand would leave the
    /// shop with a configured target and no published file.
    #[test]
    fn a_folder_that_does_not_exist_yet_is_created() {
        with_publish_folder("cenovnik-local-target-mkdir", |folder| {
            let nested = folder.join("javno").join("cenovnik");
            LocalFolderTarget::new(&nested)
                .publish(&render_csv(&[row_priced(100)]), OUTLET, NOW)
                .expect("publish");
            assert_eq!(entries(&nested), [OUTLET_FILE]);
        });
    }

    /// A target that cannot write is the routine case — an unmounted drive, a
    /// folder the shop deleted, a name that is really a file. It has to surface
    /// as an `AppError` the publish path can log, because that path runs after
    /// the price write has committed and must never panic on it.
    #[test]
    fn a_folder_that_cannot_be_created_is_an_error_rather_than_a_panic() {
        with_publish_folder("cenovnik-local-target-unwritable", |folder| {
            let occupied = folder.join("zauzeto");
            std::fs::write(&occupied, b"ovo nije folder").expect("the blocker should write");

            let error = LocalFolderTarget::new(&occupied)
                .publish(&render_csv(&[row_priced(100)]), OUTLET, NOW)
                .expect_err("a folder that is really a file cannot take the cenovnik");
            assert_eq!(error.code(), "file_system_error");
        });
    }

    /// Req. 13 has no implementation to test this cycle — the hosted endpoint is
    /// the founder decision req. 15 leaves open — so what ships is the contract
    /// itself, written where whoever implements that endpoint will read it.
    /// A contract nothing asserts is a comment, and comments get deleted.
    ///
    /// The rules are looked for in the trait's own doc block rather than
    /// anywhere in the file, so moving them into unrelated prose does not
    /// satisfy this.
    #[test]
    fn the_fetchability_contract_is_written_into_the_publish_target_trait() {
        const SOURCE: &str = include_str!("cenovnik.rs");

        let before = SOURCE
            .split("pub trait PublishTarget")
            .next()
            .expect("the trait must be in this file");
        let documented = before
            .lines()
            .rev()
            .take_while(|line| line.trim_start().starts_with("///"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();

        for rule in [
            "login",
            "captcha",
            "bot",
            "javascript",
            "robots.txt",
            "rate limit",
            "stable url",
            "content-type",
        ] {
            assert!(
                documented.contains(rule),
                "čl. 6 st. 5 is a duty to ENABLE, so every publish target owes the reader this \
                 rule and the trait doc has lost it: {rule}"
            );
        }
    }
}
