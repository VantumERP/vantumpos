//! SW-12 — republish on write (req. 11), and the archive that keeps what was
//! published (req. 14).
//!
//! Čl. 6 st. 3 obliges the trader to keep the published cenovnik matching the
//! outlet's current prices *„u realnom vremenu“*, so publication hangs off the
//! write that moved a price. A nightly batch is a defect against st. 3, not a
//! simplification.
//!
//! The trigger is the offered-price log (migration v9), not a second source of
//! truth: `record_offered_price_change` already decides what counts as a change
//! to what the shop offers — a new price, an article going off the shelf, one
//! coming back — and every write path republishes exactly when it recorded one.
//! The catalog path adds the one price that log does not watch: the jedinična
//! cena, which is a published price under čl. 6 st. 1/st. 2 and moves with the
//! v19 package-content pair rather than with `sale_price_minor`.
//!
//! **Four paths move an offered price and all four call in here** —
//! `commands::catalog` (create/update/deactivate), `campaigns` (activation, a
//! markdown step, an ending), `importer` (bulk create and update) and
//! `kep_storno::post_nivelacija`. A price the till charges but the published
//! file does not carry is the mismatch st. 3 exists for, whichever path moved
//! it.
//!
//! **The archive half.** Čl. 6 st. 5 obliges the trader to enable a comparison
//! of *„prethodno objavljenih cena“* with the realtime ones, so a publication is
//! never overwritten by the next one: [`list_snapshots`] reads an outlet's
//! lineage newest first and [`read_snapshot`] gives back the file byte for byte.
//! [`purge_expired_snapshots`] is the other end of it — ZZP čl. 213 bars the
//! prekršaj two years from commission, and after that a superseded file answers
//! nothing. It is the **only** code in this crate allowed to delete a published
//! cenovnik, and it never reaches an outlet's current one.
//!
//! Legal authority: `docs/REMAINING-SW-VERIFIED-RULES.md` §2b, §3 V2, §4 reqs.
//! 11, 14. Design: `docs/superpowers/specs/2026-08-01-sw12-cenovnik-design.md`
//! §2, §3.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::cenovnik::{
    content_hash, published_prices, render_csv, CenovnikRow, LocalFolderTarget, NotConfigured,
    PublishOutcome, PublishTarget,
};
use crate::commands::settings::{save_json_setting, CompanySettings, COMPANY_SETTINGS_KEY};
use crate::retention::{
    assert_never_purge_intact, expiry_cutoff, is_purgeable, load_policy, never_purge_row_counts,
    RecordClass, CENOVNIK_ARCHIVE_RETENTION_YEARS,
};
use crate::state::AppState;

/// The frozen archive key, minted on the first publish. Its own `settings` row
/// rather than a column on `cenovnik_snapshots`: v19 is the plan's only new
/// schema, and `settings` is already the generic per-shop key/value store.
const OUTLET_SETTINGS_KEY: &str = "cenovnik_prodajno_mesto";

/// Where the shop has said its cenovnik should go. Its own `settings` row, as
/// every other per-shop choice is.
const PUBLISH_TARGET_SETTINGS_KEY: &str = "cenovnik_publish_target";

/// The shop's answer to „where does the published file go“ (req. 15).
///
/// Req. 15 asks for a hosted endpoint — a stable public URL per prodajni
/// objekat — and **that is a founder decision, not an engineering one**: a
/// hosted endpoint means Actaer runs a public service carrying the shop's
/// prices. So this cycle ships the two states it can honestly ship, and the
/// hosted variant is a third arm added here when that decision lands, without
/// touching a single write path.
///
/// A shop that already has a website discharges čl. 6 st. 2 today by pointing
/// [`PublishTargetSettings::LocalFolder`] at the folder that site serves.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PublishTargetSettings {
    /// Nothing receives the file. The snapshot is still generated and archived
    /// — an honest state, not a failure, and **never a statement that the shop
    /// is in breach**: whether a trader with no website must create one is
    /// unresolved (§2b).
    #[default]
    NotConfigured,
    /// A folder on this machine: one the shop's web host serves, one a sync
    /// client mirrors, or simply the one it uploads from by hand.
    #[serde(rename_all = "camelCase")]
    LocalFolder { folder: String },
}

/// Republishes after a price move that has ALREADY COMMITTED.
///
/// Returns nothing, and that is the point: the catalog, campaign, import and
/// nivelacija paths all publish after their own `tx.commit()`, so by the time
/// this can fail the new price is durable. Reporting it as a failed save would
/// tell the operator to retry a write that happened — and on a product create
/// the retry then hits the UNIQUE constraint on the šifra. Čl. 6 is not a reason
/// a till cannot change a price, so a publish failure is logged and the price
/// stands; the next price write repairs the published file, exactly as it
/// repairs a crash between the commit and the publish.
///
/// **The single place the publish target is chosen**, so req. 15's founder
/// decision lands here once rather than at each of the four write paths — and
/// so a target that cannot be read is one more thing that fails toward „the
/// file was archived but went nowhere“ rather than toward a failed sale.
pub fn republish_after_price_move(connection: &Connection, now: &str) {
    let target = match load_publish_target(connection) {
        Ok(settings) => configured_target(&settings),
        Err(error) => {
            log::warn!("Mesto objave cenovnika nije pročitano, fajl se samo arhivira: {error}");
            Box::new(NotConfigured) as Box<dyn PublishTarget>
        }
    };

    if let Err(error) = publish_current(connection, target.as_ref(), now) {
        log::warn!("Cenovnik nije ponovo objavljen posle promene cene: {error}");
    }
}

/// The configured setting as the thing that actually writes the file.
fn configured_target(settings: &PublishTargetSettings) -> Box<dyn PublishTarget> {
    match settings {
        PublishTargetSettings::NotConfigured => Box::new(NotConfigured),
        PublishTargetSettings::LocalFolder { folder } => Box::new(LocalFolderTarget::new(folder)),
    }
}

/// What the shop has configured, read off the connection the publish path
/// already holds.
///
/// A settings row that will not parse answers [`PublishTargetSettings::
/// NotConfigured`] rather than failing: this is read on a path that runs after
/// a price write has committed, and a corrupt row must cost the shop its
/// publication, never its sale.
fn load_publish_target(connection: &Connection) -> Result<PublishTargetSettings, AppError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![PUBLISH_TARGET_SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored
        .and_then(|value| serde_json::from_str::<PublishTargetSettings>(&value).ok())
        .unwrap_or_default())
}

/// What the shop has configured (req. 15), for the settings panel.
///
/// Not admin-gated: knowing where the shop's own published prices go is not a
/// privilege, and čl. 6 st. 5 wants that file findable. Changing it is
/// [`save_publish_target`], which is.
pub fn publish_target(state: &AppState) -> Result<PublishTargetSettings, AppError> {
    load_publish_target(&state.db().open()?)
}

/// The ZZP čl. 6 duty and its čl. 210 fine, resolved against the **stored**
/// legal form (req. 18).
///
/// The panel that renders this may not author a figure — every statutory sum in
/// this crate lives in `legal.rs` — and it may not pick a tier either: an unset
/// legal form yields `penalty: None`, and a screen that filled that silence with
/// a plausible number would put the pravno-lice 200.000 in front of a
/// preduzetnik, which is precisely what req. 18 forbids.
///
/// Not admin-gated. Reading what the law asks of the shop is not a privilege,
/// and the same reasoning keeps [`publish_target`] and [`list_snapshots`] open.
pub fn cenovnik_notice(state: &AppState) -> Result<crate::legal::LegalNotice, AppError> {
    Ok(crate::legal::cenovnik_not_published(
        &crate::commands::settings::load_shop_profile(state)?,
    ))
}

/// The prodajni objekat the archive is keyed on — the frozen key once anything
/// has been published, otherwise what the shop's settings identify it as today.
///
/// **`None` is the state [`publish_current`] answers `Ok(None)` for**, and the
/// reason this is readable at all: nothing is rendered, nothing is archived and
/// nothing is published, on every price write, until the shop names itself. A
/// fresh install sits exactly there — `load_json_setting` hands back
/// `CompanySettings::default()` without persisting it, and `save_company_settings`
/// is the only writer of that row — so a panel that could not see this state
/// would tell the owner a file is made on every price change while nothing ever
/// is. That is a promise the code does not keep, on a screen a shop owner reads.
///
/// Not admin-gated, for the same reason [`publish_target`] is not: knowing
/// whether the shop's own cenovnik is being made is not a privilege.
pub fn outlet(state: &AppState) -> Result<Option<String>, AppError> {
    let connection = state.db().open()?;
    if let Some(frozen) = frozen_outlet(&connection)? {
        return Ok(Some(frozen));
    }
    identified_outlet(&connection)
}

/// Chooses where the cenovnik is published, from the owner's settings screen.
///
/// **Admin only.** Where the shop's published prices go is the thing čl. 6
/// st. 4 then binds it to, and an operator who could move it — or silently turn
/// it off — could put the shop out of step with st. 3 from a screen the owner
/// never opened.
///
/// The folder is validated but not written to here: a drive that is unmounted
/// this minute is not a wrong setting, and refusing to save it would leave the
/// shop unable to configure the target it uses. The publish path reports what
/// actually happened, and Task 8's panel is where the operator sees a
/// publication that did not land.
pub fn save_publish_target(
    state: &AppState,
    request: PublishTargetSettings,
) -> Result<PublishTargetSettings, AppError> {
    super::auth::require_admin(state)?;

    let settings = match request {
        PublishTargetSettings::NotConfigured => PublishTargetSettings::NotConfigured,
        PublishTargetSettings::LocalFolder { folder } => {
            let folder = folder.trim().to_string();
            if folder.is_empty() {
                return Err(AppError::validation(
                    "Folder za objavu cenovnika je obavezan.",
                    serde_json::json!({ "field": "folder" }),
                ));
            }
            if !Path::new(&folder).is_absolute() {
                // A relative path resolves against whatever directory the app
                // was launched from, so the shop would be told the cenovnik is
                // published and be unable to say where.
                return Err(AppError::validation(
                    "Putanja do foldera mora biti puna putanja, na primer „/Users/ana/cenovnik“.",
                    serde_json::json!({ "field": "folder" }),
                ));
            }
            PublishTargetSettings::LocalFolder { folder }
        }
    };

    save_json_setting(state, PUBLISH_TARGET_SETTINGS_KEY, &settings)?;
    Ok(settings)
}

/// Renders the outlet's cenovnik from the catalog, archives it as a new
/// immutable snapshot, and offers it to `target`. Returns the snapshot's id.
///
/// `None` means the shop has not identified a prodajni objekat yet, so there was
/// nothing to key an archive lineage on — see [`prodajno_mesto`]. That is not an
/// error: a price save must not fail because the settings are half-filled.
///
/// **Call this after the price write has committed.** The archive is evidence of
/// what the shop offered, and čl. 6 st. 4 binds it to the published prices, so a
/// target must never be handed a price the catalog then rolled back. The cost of
/// that ordering is that a crash between the two leaves the published file one
/// price behind — which is what a stale cenovnik in fact is, and the next price
/// write repairs it.
///
/// The target is a parameter rather than a lookup so this can land ahead of
/// Task 7's real targets; [`crate::cenovnik::NotConfigured`] is today's default.
pub fn publish_current(
    connection: &Connection,
    target: &dyn PublishTarget,
    now: &str,
) -> Result<Option<i64>, AppError> {
    let Some(prodajno_mesto) = prodajno_mesto(connection, now)? else {
        return Ok(None);
    };

    let rows = load_offered_rows(connection)?;
    let body = render_csv(&rows);
    let hash = content_hash(&body);
    let row_count = i64::try_from(rows.len()).unwrap_or(i64::MAX);

    // A plain INSERT — never `INSERT OR REPLACE`, never `ON CONFLICT DO UPDATE`.
    // SQLite runs REPLACE as a DELETE plus an INSERT, which no v19 immutability
    // trigger sees, so an upsert here would silently rewrite an already
    // published body and drop its `published_at`. Two writes in the same second
    // are two rows; the later id wins as current.
    connection.execute(
        "INSERT INTO cenovnik_snapshots (
            prodajno_mesto, generated_at, row_count, content_hash, body, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?2)",
        params![prodajno_mesto, now, row_count, hash, body],
    )?;
    let snapshot_id = connection.last_insert_rowid();

    if let PublishOutcome::Published { target } = target.publish(&body, &prodajno_mesto, now)? {
        // The one field written after the fact, and the v19 trigger allows it
        // once: a snapshot is generated first and only then accepted by a target.
        connection.execute(
            "UPDATE cenovnik_snapshots
             SET published_at = ?1, published_target = ?2
             WHERE id = ?3 AND published_at IS NULL",
            params![now, target, snapshot_id],
        )?;
    }

    Ok(Some(snapshot_id))
}

/// „A later cenovnik exists for this row's prodajno mesto“ — a greater
/// `generated_at`, or on a same-second tie a greater id.
///
/// Its negation is v19's reading of *the outlet's current cenovnik*, and it is
/// written once here so the archive's list, its reader and its purge cannot
/// drift apart. There is no stored pointer to compare it against: a second
/// source of truth is exactly what two writers leave aimed at a snapshot that is
/// no longer the newest, and čl. 6 st. 4 binds the shop to whatever the current
/// one says.
///
/// The correlated subquery names the outer table explicitly, so this fragment
/// composes into any statement whose FROM (or DELETE) target is
/// `cenovnik_snapshots`.
const A_LATER_SNAPSHOT_EXISTS: &str = "EXISTS (
            SELECT 1 FROM cenovnik_snapshots later
             WHERE later.prodajno_mesto = cenovnik_snapshots.prodajno_mesto
               AND (later.generated_at > cenovnik_snapshots.generated_at
                    OR (later.generated_at = cenovnik_snapshots.generated_at
                        AND later.id > cenovnik_snapshots.id)))";

/// One published cenovnik as the archive lists it — everything except the file
/// itself, which [`read_snapshot`] fetches on demand. A shop that has published
/// on every price move for a year has a lot of bodies, and a list screen needs
/// none of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CenovnikSnapshotSummary {
    pub id: i64,
    pub prodajno_mesto: String,
    pub generated_at: String,
    pub row_count: i64,
    pub content_hash: String,
    /// `None` while the file was generated and archived but no target accepted
    /// it — the honest state req. 15 leaves open, not a failure.
    pub published_at: Option<String>,
    pub published_target: Option<String>,
    /// The outlet's newest snapshot: the file čl. 6 st. 4 binds the shop to
    /// today. **Derived** — see [`A_LATER_SNAPSHOT_EXISTS`].
    pub current: bool,
}

/// A published cenovnik together with the file itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CenovnikSnapshotDetail {
    pub snapshot: CenovnikSnapshotSummary,
    /// The rendered file, byte for byte as it was archived and as any target
    /// received it — BOM, CRLF and diacritics included. Čl. 6 st. 5 asks the
    /// trader to enable a comparison of the previously published prices with the
    /// realtime ones, and a body that came back normalised would answer a
    /// different question than „what did this shop publish“.
    pub body: String,
}

/// The outlet's archive, newest first (req. 14).
///
/// Čl. 6 st. 2 publishes *„posebno za svaki prodajni objekat“*, so the
/// comparison st. 5 asks for runs inside one outlet's lineage — and the outlet
/// is the frozen archive key, never a parameter a caller could get wrong. A shop
/// that has published nothing yet has no key and gets an empty list rather than
/// an error: there is no archive to read, which is not a fault.
///
/// **Not admin-gated, on purpose.** Čl. 6 st. 5 is a duty to *enable* the
/// comparison and req. 13 requires the published file to be anonymously
/// fetchable; putting a role check in front of the shop's own copy of a document
/// the law wants public would be theatre. The archive holds no personal data —
/// see [`crate::retention::RecordClass::personal_data`].
pub fn list_snapshots(state: &AppState) -> Result<Vec<CenovnikSnapshotSummary>, AppError> {
    let connection = state.db().open()?;
    let Some(outlet) = frozen_outlet(&connection)? else {
        return Ok(Vec::new());
    };

    let mut statement = connection.prepare(
        "SELECT id, prodajno_mesto, generated_at, row_count, content_hash,
                published_at, published_target
         FROM cenovnik_snapshots
         WHERE prodajno_mesto = ?1
         ORDER BY generated_at DESC, id DESC",
    )?;
    let mut rows = statement
        .query_map(params![outlet], |row| {
            Ok(CenovnikSnapshotSummary {
                id: row.get(0)?,
                prodajno_mesto: row.get(1)?,
                generated_at: row.get(2)?,
                row_count: row.get(3)?,
                content_hash: row.get(4)?,
                published_at: row.get(5)?,
                published_target: row.get(6)?,
                // Filled in below rather than per row: the ORDER BY has already
                // decided which one it is, and asking the database again row by
                // row would be a second answer to a question already settled.
                current: false,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(newest) = rows.first_mut() {
        newest.current = true;
    }

    Ok(rows)
}

/// One archived cenovnik with its file, or `None` when no such snapshot exists.
///
/// By id and not by outlet: a snapshot's id is what a divergence record or an
/// inspector's question names, and an id that belongs to an older lineage is
/// still this shop's own publication.
pub fn read_snapshot(
    state: &AppState,
    snapshot_id: i64,
) -> Result<Option<CenovnikSnapshotDetail>, AppError> {
    let connection = state.db().open()?;
    let found = connection
        .query_row(
            &format!(
                "SELECT id, prodajno_mesto, generated_at, row_count, content_hash,
                        published_at, published_target, body, NOT {A_LATER_SNAPSHOT_EXISTS}
                 FROM cenovnik_snapshots
                 WHERE id = ?1"
            ),
            params![snapshot_id],
            |row| {
                Ok(CenovnikSnapshotDetail {
                    snapshot: CenovnikSnapshotSummary {
                        id: row.get(0)?,
                        prodajno_mesto: row.get(1)?,
                        generated_at: row.get(2)?,
                        row_count: row.get(3)?,
                        content_hash: row.get(4)?,
                        published_at: row.get(5)?,
                        published_target: row.get(6)?,
                        current: row.get::<_, i64>(8)? != 0,
                    },
                    body: row.get(7)?,
                })
            },
        )
        .optional()?;

    Ok(found)
}

/// The outlet's current published cenovnik, reduced to what the till compares a
/// rung price against (req. 12).
///
/// Carries the snapshot's identity as well as its prices, because a divergence
/// record that did not name the file it was measured against would be an
/// accusation with no exhibit: the archive holds many files for one outlet and
/// čl. 6 st. 4 binds the shop only to the one in force at the time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCenovnik {
    pub snapshot_id: i64,
    pub generated_at: String,
    pub content_hash: String,
    prices: BTreeMap<String, i64>,
}

impl PublishedCenovnik {
    /// The prodajna cena this file publishes for `sifra`, or `None` when the file
    /// does not carry the article at all — a new product the shop has not
    /// republished for yet has no published price, and nothing may be inferred
    /// for it.
    pub fn prodajna_cena(&self, sifra: &str) -> Option<i64> {
        self.prices.get(sifra).copied()
    }
}

/// Reads the outlet's current cenovnik and the prices in it.
///
/// `None` means there is nothing to compare against — no prodajni objekat minted
/// yet, or nothing published for it. **That is not an error and never a reason a
/// till cannot sell**: a shop that has published nothing has made no čl. 6 st. 4
/// promise to depart from, and the hosting question (req. 15) is deliberately
/// still open.
///
/// „Current“ is the same reading the archive's list and its purge take — the
/// newest `generated_at`, a same-second tie broken by the larger id — so the file
/// the guard measures against is the file the panel calls current.
pub fn current_published_cenovnik(
    connection: &Connection,
) -> Result<Option<PublishedCenovnik>, AppError> {
    let Some(outlet) = frozen_outlet(connection)? else {
        return Ok(None);
    };

    let found = connection
        .query_row(
            "SELECT id, generated_at, content_hash, body
             FROM cenovnik_snapshots
             WHERE prodajno_mesto = ?1
             ORDER BY generated_at DESC, id DESC
             LIMIT 1",
            params![outlet],
            |row| {
                Ok(PublishedCenovnik {
                    snapshot_id: row.get(0)?,
                    generated_at: row.get(1)?,
                    content_hash: row.get(2)?,
                    prices: published_prices(&row.get::<_, String>(3)?),
                })
            },
        )
        .optional()?;

    Ok(found)
}

/// The time-driven cut over the archive (req. 14).
///
/// **This is the only code in the crate permitted to delete a published
/// cenovnik**, and `the_retention_purge_is_the_only_code_that_deletes_a_published_cenovnik`
/// is what keeps it that way. v19 carries no DELETE trigger on purpose — čl. 213
/// gives the archive a two-year limitation rather than a `trajno` duty, so the
/// purge has to be able to reach an expired snapshot — and no trigger can tell a
/// purge from a cover-up, so the constraint is a property of the code instead.
///
/// **Two clocks and one absolute rule.** The shared `retention_policies` row is
/// the first clock: a legal hold, or a rok the shop moved forward, refuses the
/// cut for the whole class. Each snapshot's own `generated_at` is the second,
/// through [`expiry_cutoff`] — the class floor alone would release every row in
/// the class from its anniversary onward, which is the axis §4d says must never
/// be inverted. The rule no date overrides is that an outlet's **current**
/// cenovnik stays: čl. 6 st. 4 binds the shop to the file in force, and st. 5's
/// comparison has nothing to compare against once it is gone. A shop that has
/// not moved a price in three years still has its published cenovnik.
///
/// The [`never_purge_row_counts`] / [`assert_never_purge_intact`] fence wraps the
/// whole transaction, as it does on every other purge path: a future edit that
/// finds a way to delete a `trajno` row here aborts the cut rather than
/// committing it.
pub fn purge_expired_snapshots(state: &AppState, now: &str) -> Result<usize, AppError> {
    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;
    let never_purge_before = never_purge_row_counts(&tx)?;

    let policy = load_policy(&tx, RecordClass::CenovnikArchive)?;
    // A legal hold, a `trajno` flag or a class floor the day has not reached each
    // refuse on their own — see `retention::is_purgeable`.
    if !is_purgeable(&policy, now) {
        return Ok(0);
    }
    // An unreadable day yields no cutoff, and no cutoff means keep.
    let Some(cutoff) = expiry_cutoff(now, CENOVNIK_ARCHIVE_RETENTION_YEARS) else {
        return Ok(0);
    };

    let removed = tx.execute(
        &format!(
            "DELETE FROM cenovnik_snapshots
              WHERE substr(generated_at, 1, 10) < ?1
                AND {A_LATER_SNAPSHOT_EXISTS}"
        ),
        params![cutoff],
    )?;

    assert_never_purge_intact(&tx, &never_purge_before)?;
    tx.commit()?;

    Ok(removed)
}

/// The outlet the archive is keyed on: derived from the shop's own settings the
/// first time anything is published, and frozen from then on.
///
/// **Never accepted from the frontend.** `prodajno_mesto` is free text with no
/// foreign key, so one typo starts a second archive lineage and leaves the real
/// outlet with no published snapshot for the till guard to compare against.
///
/// **And never re-derived either, for the same reason.** Settings are editable:
/// a shop that corrects an address typo, or fills the address in after
/// publishing under its name alone, would silently start that second lineage
/// itself — every prior snapshot orphaned under the old key, the newest-first
/// lookup empty until the next price write, and the čl. 6 st. 5 comparison of
/// *„prethodno objavljenih cena“* with the realtime ones broken across the
/// boundary. The v19 trigger makes `prodajno_mesto` immutable, so a split
/// lineage could never be re-joined afterwards. The key is therefore minted once
/// and kept; what the shop is called today is read from the settings, which are
/// where that question belongs. A genuine move to a different prodajni objekat
/// is a new lineage on purpose and needs an explicit operator action, not a
/// silent consequence of an edit.
///
/// The address is the prodajni objekat — Zakon o trgovini čl. 2 t. 3, a
/// physically and functionally unified space — and is already how `kalkulacije`
/// records it, so the two modules name the same outlet the same way. A shop that
/// left the address blank is still identified by its name, which
/// `save_company_settings` refuses to leave empty.
///
/// `None` means neither is on file. The v19 CHECK refuses a blank outlet, and a
/// placeholder lineage would be a claim about which shop published what, so
/// nothing is generated at all — and nothing is frozen either, so the shop can
/// still identify itself later.
fn prodajno_mesto(connection: &Connection, now: &str) -> Result<Option<String>, AppError> {
    if let Some(frozen) = frozen_outlet(connection)? {
        return Ok(Some(frozen));
    }

    let Some(identified) = identified_outlet(connection)? else {
        return Ok(None);
    };

    // OR IGNORE, not an upsert: two publishes racing to mint the key must
    // converge on ONE lineage, and whichever landed first is it. Re-read rather
    // than trust the local value, so the loser adopts the winner's key.
    connection.execute(
        "INSERT OR IGNORE INTO settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)",
        params![
            OUTLET_SETTINGS_KEY,
            serde_json::json!(identified).to_string(),
            now
        ],
    )?;
    frozen_outlet(connection)
}

/// The minted key, or `None` before the first publish.
fn frozen_outlet(connection: &Connection) -> Result<Option<String>, AppError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![OUTLET_SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored
        .and_then(|value| serde_json::from_str::<String>(&value).ok())
        .map(|outlet| outlet.trim().to_string())
        .filter(|outlet| !outlet.is_empty()))
}

/// What the shop's settings say the outlet is called right now.
fn identified_outlet(connection: &Connection) -> Result<Option<String>, AppError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![COMPANY_SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    // A corrupt settings row must not take a price save down with it; it lands
    // in the „no outlet identified“ branch instead, which the panel can show.
    let Some(company) =
        stored.and_then(|value| serde_json::from_str::<CompanySettings>(&value).ok())
    else {
        return Ok(None);
    };

    let identified = [company.address.as_str(), company.shop_name.as_str()]
        .into_iter()
        .map(str::trim)
        .find(|candidate| !candidate.is_empty())
        .map(str::to_string);
    Ok(identified)
}

/// The catalog as the published file sees it: what the outlet actually offers.
///
/// Inactive articles are left out — an article off the shelf is not offered, so
/// it has no price to publish, which is the same reading `price_history` takes.
///
/// No ORDER BY: `render_csv` sorts by šifra so that the same catalog always
/// renders the same bytes and therefore the same `content_hash`, whatever order
/// this SELECT happens to return.
fn load_offered_rows(connection: &Connection) -> Result<Vec<CenovnikRow>, AppError> {
    let mut statement = connection.prepare(
        "SELECT p.sku,
                p.barcode,
                p.name,
                p.unit_of_measure,
                p.sale_price_minor,
                p.jedinicna_cena_jedinica,
                p.jedinicna_cena_sadrzaj_milli,
                -- datum_azuriranja is the date the published price took effect,
                -- so it comes from the offered-price log; products.updated_at
                -- moves on any edit. A product with no logged price falls back
                -- to its own stamp rather than publishing an empty date.
                COALESCE(
                    (SELECT MAX(h.effective_from)
                     FROM price_history h
                     WHERE h.product_id = p.id AND h.price_minor IS NOT NULL),
                    p.updated_at
                )
         FROM products p
         WHERE p.active = 1",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(CenovnikRow {
                sifra: row.get(0)?,
                barkod: row.get(1)?,
                naziv: row.get(2)?,
                jedinica_mere: row.get(3)?,
                prodajna_cena_minor: row.get(4)?,
                jedinicna_cena_jedinica: row.get(5)?,
                jedinicna_cena_sadrzaj_milli: row.get(6)?,
                datum_azuriranja: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn cenovnik_list_snapshots(
    state: State<'_, AppState>,
) -> Result<Vec<CenovnikSnapshotSummary>, CommandError> {
    list_snapshots(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn cenovnik_get_snapshot(
    state: State<'_, AppState>,
    snapshot_id: i64,
) -> Result<Option<CenovnikSnapshotDetail>, CommandError> {
    read_snapshot(state.inner(), snapshot_id).map_err(Into::into)
}

#[tauri::command]
pub fn cenovnik_get_notice(
    state: State<'_, AppState>,
) -> Result<crate::legal::LegalNotice, CommandError> {
    cenovnik_notice(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn cenovnik_get_outlet(state: State<'_, AppState>) -> Result<Option<String>, CommandError> {
    outlet(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn cenovnik_get_publish_target(
    state: State<'_, AppState>,
) -> Result<PublishTargetSettings, CommandError> {
    publish_target(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn cenovnik_set_publish_target(
    state: State<'_, AppState>,
    request: PublishTargetSettings,
) -> Result<PublishTargetSettings, CommandError> {
    save_publish_target(state.inner(), request).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use rusqlite::params;

    use super::{
        cenovnik_notice, current_published_cenovnik, list_snapshots, outlet, publish_current,
        publish_target, purge_expired_snapshots, read_snapshot, republish_after_price_move,
        save_publish_target, PublishTargetSettings,
    };
    use crate::app_error::AppError;
    use crate::cenovnik::{published_file_name, NotConfigured, PublishOutcome, PublishTarget};
    use crate::commands::catalog::{
        create_product, set_product_active, update_product, SaveProductRequest,
    };
    use crate::db::{test_database_path, Db};
    use crate::retention::{
        extend_retain_until, seed_retention_policies, RecordClass, CENOVNIK_ARCHIVE_RETENTION_YEARS,
    };
    use crate::state::AppState;

    const OUTLET: &str = "Bulevar oslobođenja 1, Novi Sad";

    /// The day the shop's retention classes were recorded. Deliberately far
    /// behind the `now` the archive tests publish at, so the class-wide floor is
    /// already reached and each test is exercising the **per-snapshot** čl. 213
    /// cutoff rather than the seeding anniversary.
    const CLASSES_SEEDED_AT: &str = "2024-01-01T08:00:00Z";

    fn with_database(test_name: &str, test: impl FnOnce(&Db)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            seed(&db);
            test(&db);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// The same shop, plus the shared retention table the archive resolves its
    /// rok through.
    fn with_archive(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            seed(&db);
            let state = AppState::new(db);
            seed_retention_policies(&state, CLASSES_SEEDED_AT).expect("classes should seed");
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// The same shop reached through an `AppState`, so the admin gate on the
    /// publish target (Task 7) can be exercised.
    fn with_shop(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            seed(&db);
            test(&AppState::new(db));
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// A folder of this test's own, removed afterwards — `std::env::temp_dir`
    /// plus a nanosecond suffix, as `db::test_database_path` does it.
    fn with_publish_folder(test_name: &str, test: impl FnOnce(&std::path::Path)) {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let folder = std::env::temp_dir().join(format!("vantumpos-{test_name}-{unique}"));
        std::fs::create_dir_all(&folder).expect("the publish folder should be created");

        test(&folder);

        std::fs::remove_dir_all(&folder).expect("the publish folder should be removed");
    }

    fn sign_in_admin(state: &AppState) {
        let admin = admin_id(state.db());
        state
            .set_session_user_id(admin)
            .expect("admin session should set");
    }

    fn sign_in_cashier(state: &AppState) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('marko', 'Marko Marković', 'cashier',
                         '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier)
            .expect("cashier session should set");
    }

    fn seed(db: &Db) {
        let connection = db.open().expect("database should open");
        connection
            .execute(
                "INSERT INTO categories (id, name, created_at, updated_at)
                 VALUES (1, 'Piće', '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("category should insert");
        connection
            .execute(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("tax rate should insert");
        // A 0,75 l bottle: the v19 unit-price pair is populated so the published
        // file has a jedinična cena to carry (req. 10).
        connection
            .execute(
                "INSERT INTO products (
                    id, name, sku, barcode, category_id, unit_of_measure,
                    sale_price_minor, purchase_price_minor, tax_rate_id,
                    minimum_stock_milli, active,
                    jedinicna_cena_jedinica, jedinicna_cena_sadrzaj_milli,
                    created_at, updated_at
                 )
                 VALUES (1, 'Sok od jabuke 0,75 l', 'SOK-075', '8600000000010', 1, 'kom',
                         27900, 15000, 1, 0, 1, 'l', 750,
                         '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("offered product should insert");
        connection
            .execute(
                "INSERT INTO products (
                    id, name, sku, barcode, category_id, unit_of_measure,
                    sale_price_minor, purchase_price_minor, tax_rate_id,
                    minimum_stock_milli, active, created_at, updated_at
                 )
                 VALUES (2, 'Arhivirani artikal', 'ARH-1', NULL, 1, 'kom',
                         100000, 70000, 1, 0, 0,
                         '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                [],
            )
            .expect("archived product should insert");
        set_company(db, OUTLET, "Butik Ana");
    }

    fn set_company(db: &Db, address: &str, shop_name: &str) {
        let connection = db.open().expect("database should open");
        let value_json = serde_json::json!({
            "shopName": shop_name,
            "address": address,
            "pib": "",
            "registrationNumber": "",
            "phone": "",
            "logoPath": null,
            "currency": "RSD",
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO settings (key, value_json, updated_at)
                 VALUES ('company', ?1, '2026-06-18T10:00:00Z')
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![value_json],
            )
            .expect("company settings should save");
    }

    fn admin_id(db: &Db) -> i64 {
        db.open()
            .expect("database should open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist")
    }

    fn product_request(sale_price_minor: i64) -> SaveProductRequest {
        SaveProductRequest {
            name: "Sok od jabuke 0,75 l".to_string(),
            sku: "SOK-075".to_string(),
            barcode: Some("8600000000010".to_string()),
            category_id: Some(1),
            unit_of_measure: "kom".to_string(),
            sale_price_minor,
            purchase_price_minor: 15000,
            tax_rate_id: 1,
            minimum_stock_milli: 0,
            allow_negative_stock: false,
            active: true,
            perishable: false,
            perishable_justification: None,
            external_source: None,
            manufacturer_name: None,
            importer_name: None,
            country_of_origin: None,
            official_goods_code: None,
            barcode_kind: None,
            // The seeded 0,75 l bottle round-trips its unit-price pair: this
            // request is a FULL replacement of the row, exactly like every other
            // optional column on it, so a caller that drops the pair clears it.
            jedinicna_cena_jedinica: Some("l".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(750),
        }
    }

    /// One article's row in the outlet's newest snapshot, split into fields.
    fn latest_fields(db: &Db, sifra: &str) -> Vec<String> {
        let body = snapshots(db).pop().expect("a snapshot").body;
        data_rows(&body)
            .into_iter()
            .find(|row| row.starts_with(&format!("{sifra};")))
            .unwrap_or_else(|| panic!("{sifra} should be in the published file: {body:?}"))
            .split(';')
            .map(str::to_string)
            .collect()
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SnapshotRow {
        id: i64,
        prodajno_mesto: String,
        generated_at: String,
        row_count: i64,
        content_hash: String,
        body: String,
        published_at: Option<String>,
        published_target: Option<String>,
    }

    fn snapshots(db: &Db) -> Vec<SnapshotRow> {
        let connection = db.open().expect("database should open");
        let mut statement = connection
            .prepare(
                "SELECT id, prodajno_mesto, generated_at, row_count, content_hash, body,
                        published_at, published_target
                 FROM cenovnik_snapshots
                 ORDER BY id",
            )
            .expect("snapshot query should prepare");
        let rows = statement
            .query_map([], |row| {
                Ok(SnapshotRow {
                    id: row.get(0)?,
                    prodajno_mesto: row.get(1)?,
                    generated_at: row.get(2)?,
                    row_count: row.get(3)?,
                    content_hash: row.get(4)?,
                    body: row.get(5)?,
                    published_at: row.get(6)?,
                    published_target: row.get(7)?,
                })
            })
            .expect("snapshot query should run");
        rows.collect::<Result<Vec<_>, _>>()
            .expect("snapshot rows should read")
    }

    /// The v19 reading of „the outlet's current cenovnik“: newest first, a
    /// same-second tie broken by the larger id.
    fn current_snapshot(db: &Db, prodajno_mesto: &str) -> Option<i64> {
        let connection = db.open().expect("database should open");
        connection
            .query_row(
                "SELECT id FROM cenovnik_snapshots
                 WHERE prodajno_mesto = ?1
                 ORDER BY generated_at DESC, id DESC
                 LIMIT 1",
                params![prodajno_mesto],
                |row| row.get(0),
            )
            .ok()
    }

    fn data_rows(body: &str) -> Vec<String> {
        body.lines()
            .skip(1)
            .map(|line| line.trim_end_matches('\r').to_string())
            .collect()
    }

    struct RecordingTarget {
        bodies: RefCell<Vec<String>>,
    }

    impl RecordingTarget {
        fn new() -> Self {
            Self {
                bodies: RefCell::new(Vec::new()),
            }
        }
    }

    impl PublishTarget for RecordingTarget {
        fn publish(
            &self,
            body: &str,
            prodajno_mesto: &str,
            _now: &str,
        ) -> Result<PublishOutcome, AppError> {
            self.bodies.borrow_mut().push(body.to_string());
            Ok(PublishOutcome::Published {
                target: format!("test://{prodajno_mesto}"),
            })
        }
    }

    /// Req. 11: price change → republish. Čl. 6 st. 3 wants the published file to
    /// match the current prices „u realnom vremenu“, so the republish rides on
    /// the write that moved them.
    #[test]
    fn a_price_write_publishes_a_new_snapshot() {
        with_database("a_price_write_publishes_a_new_snapshot", |db| {
            let acting = admin_id(db);
            update_product(db, 1, product_request(31_900), acting).expect("price should save");

            let archived = snapshots(db);
            assert_eq!(archived.len(), 1, "{archived:?}");
            assert_eq!(archived[0].prodajno_mesto, OUTLET);
            assert_eq!(archived[0].row_count, 1);
            assert!(
                archived[0].body.contains("319.00"),
                "the archived file must carry the price that was just saved: {:?}",
                archived[0].body
            );
        });
    }

    /// „Saving a non-price field does not republish.“ Čl. 6 st. 3 is about
    /// prices, and the price log is what decides: an edit that changes no
    /// offered price appends nothing there and publishes nothing here.
    #[test]
    fn a_non_price_edit_publishes_nothing() {
        with_database("a_non_price_edit_publishes_nothing", |db| {
            let acting = admin_id(db);
            let renamed = SaveProductRequest {
                name: "Sok od jabuke, 0,75 l".to_string(),
                minimum_stock_milli: 5_000,
                ..product_request(27_900)
            };
            update_product(db, 1, renamed, acting).expect("edit should save");

            assert!(
                snapshots(db).is_empty(),
                "an edit that moved no price is not a price change"
            );
        });
    }

    /// Req. 10 through the path a shop actually uses. Čl. 6 st. 2's second
    /// sentence pulls st. 1 in, so a cenovnik carrying only prodajna cena does
    /// not discharge the duty — and a publish path that selects a column nothing
    /// can write publishes an empty cell for every real shop. 129,00 RSD for
    /// 0,5 l is 258,00 RSD per litar.
    #[test]
    fn a_product_saved_with_a_package_content_publishes_its_unit_price() {
        with_database(
            "a_product_saved_with_a_package_content_publishes_its_unit_price",
            |db| {
                let acting = admin_id(db);
                let request = SaveProductRequest {
                    name: "Kefir 0,5 l".to_string(),
                    sku: "KEFIR-05".to_string(),
                    barcode: None,
                    jedinicna_cena_jedinica: Some("l".to_string()),
                    jedinicna_cena_sadrzaj_milli: Some(500),
                    ..product_request(12_900)
                };
                create_product(db, request, acting).expect("product should create");

                let fields = latest_fields(db, "KEFIR-05");
                assert_eq!(fields[4], "129.00", "{fields:?}");
                assert_eq!(fields[5], "258.00", "jedinična cena po litru: {fields:?}");
                assert_eq!(fields[6], "l", "{fields:?}");
            },
        );
    }

    /// The jedinična cena IS a price under čl. 6 st. 1/st. 2, and it is derived
    /// from the package content. Correcting a 0,75 l bottle to 1 l moves the
    /// published unit price without moving `sale_price_minor`, so a republish
    /// predicate that only watches the sale price would leave the file stating a
    /// unit price the shop no longer offers (st. 3, and st. 4 binds it).
    #[test]
    fn changing_only_the_package_content_republishes_the_new_unit_price() {
        with_database(
            "changing_only_the_package_content_republishes_the_new_unit_price",
            |db| {
                let acting = admin_id(db);
                let relabelled = SaveProductRequest {
                    jedinicna_cena_sadrzaj_milli: Some(1000),
                    ..product_request(27_900)
                };
                update_product(db, 1, relabelled, acting).expect("content should save");

                let archived = snapshots(db);
                assert_eq!(archived.len(), 1, "a unit price is a price: {archived:?}");
                let fields = latest_fields(db, "SOK-075");
                assert_eq!(
                    fields[4], "279.00",
                    "the sale price did not move: {fields:?}"
                );
                assert_eq!(fields[5], "279.00", "{fields:?}");
            },
        );
    }

    /// The publish runs AFTER `tx.commit()`, so by the time it can fail the
    /// price is already durable. Reporting that as a failed save tells the
    /// operator to retry a write that happened — and on a create the retry then
    /// hits the UNIQUE constraint on the šifra. Čl. 6 is not a reason a till
    /// cannot change a price.
    #[test]
    fn a_publish_failure_leaves_the_price_saved_and_the_command_ok() {
        with_database(
            "a_publish_failure_leaves_the_price_saved_and_the_command_ok",
            |db| {
                // Stands in for Task 7's disk-full / permission failures, which
                // are the routine way an already-committed price change will
                // fail to publish once a real target exists.
                db.open()
                    .expect("database should open")
                    .execute("DROP TABLE cenovnik_snapshots", [])
                    .expect("the archive should drop");

                let acting = admin_id(db);
                update_product(db, 1, product_request(31_900), acting)
                    .expect("a committed price save must not be reported as failed");

                let saved: i64 = db
                    .open()
                    .expect("database should open")
                    .query_row(
                        "SELECT sale_price_minor FROM products WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .expect("price should read");
                assert_eq!(saved, 31_900);
            },
        );
    }

    #[test]
    fn creating_a_product_publishes_the_new_offer() {
        with_database("creating_a_product_publishes_the_new_offer", |db| {
            let acting = admin_id(db);
            let request = SaveProductRequest {
                name: "Kefir 0,5 l".to_string(),
                sku: "KEFIR-05".to_string(),
                barcode: None,
                ..product_request(12_900)
            };
            create_product(db, request, acting).expect("product should create");

            let archived = snapshots(db);
            assert_eq!(archived.len(), 1, "{archived:?}");
            assert_eq!(archived[0].row_count, 2, "both offered products");
            assert!(
                archived[0].body.contains("KEFIR-05"),
                "{:?}",
                archived[0].body
            );
        });
    }

    /// Taking an article off the shelf ends its offering, so the published file
    /// must stop naming it — the same event the price log records as a gap.
    #[test]
    fn deactivating_a_product_republishes_the_file_without_it() {
        with_database(
            "deactivating_a_product_republishes_the_file_without_it",
            |db| {
                let acting = admin_id(db);
                set_product_active(db, 1, false, acting).expect("product should deactivate");

                let archived = snapshots(db);
                assert_eq!(archived.len(), 1, "{archived:?}");
                assert_eq!(archived[0].row_count, 0);
                assert!(
                    !archived[0].body.contains("SOK-075"),
                    "an article that is no longer offered has no published price: {:?}",
                    archived[0].body
                );
            },
        );
    }

    /// Two writes in the same second are the case the v19 tie-break exists for.
    /// They are two snapshots — the archive is append-only, and nothing dedupes
    /// them into one row, because the idiomatic dedupe is an upsert.
    #[test]
    fn two_writes_in_the_same_second_are_two_snapshots_and_the_later_one_is_current() {
        with_database(
            "two_writes_in_the_same_second_are_two_snapshots_and_the_later_one_is_current",
            |db| {
                let connection = db.open().expect("database should open");
                let first = publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("first publish")
                    .expect("an identified outlet publishes");
                let second = publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("second publish")
                    .expect("an identified outlet publishes");

                assert_ne!(first, second);
                let archived = snapshots(db);
                assert_eq!(archived.len(), 2, "{archived:?}");
                assert_eq!(archived[0].generated_at, archived[1].generated_at);
                assert_eq!(
                    current_snapshot(db, OUTLET),
                    Some(second),
                    "on a same-second tie the larger id is the current cenovnik"
                );
            },
        );
    }

    /// Čl. 6 st. 5 requires enabling comparison of „prethodno objavljenih cena“
    /// with the real-time ones, so the file a republish replaces must remain
    /// retrievable exactly as it was published.
    #[test]
    fn the_previous_snapshot_survives_the_republish_byte_for_byte() {
        with_database(
            "the_previous_snapshot_survives_the_republish_byte_for_byte",
            |db| {
                let connection = db.open().expect("database should open");
                publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("first publish");
                let before = snapshots(db);
                let first = before[0].clone();

                let acting = admin_id(db);
                update_product(db, 1, product_request(31_900), acting).expect("price should save");

                let after = snapshots(db);
                assert_eq!(after.len(), 2, "{after:?}");
                assert_eq!(after[0], first, "the earlier publication is untouched");
                assert!(after[0].body.contains("279.00"), "{:?}", after[0].body);
                assert!(after[1].body.contains("319.00"), "{:?}", after[1].body);
                assert_ne!(after[0].content_hash, after[1].content_hash);
            },
        );
    }

    /// SQLite runs REPLACE as a DELETE plus an INSERT, which no v19 immutability
    /// trigger sees: an upsert here would silently rewrite an already published
    /// body and drop its `published_at`. The engine cannot close that without
    /// closing the čl. 213 purge, so the write path carries the constraint — and
    /// therefore has to assert it.
    ///
    /// The needles are composed at run time so this assertion cannot match its
    /// own source text.
    #[test]
    fn the_publish_path_never_replaces_or_upserts_a_snapshot() {
        const THIS_MODULE: &str = include_str!("cenovnik.rs");
        const CATALOG: &str = include_str!("catalog.rs");

        // Statements only: the shipped half with every Rust and SQL comment line
        // dropped, so the prose explaining the rule cannot trip the rule.
        fn statements(source: &str) -> String {
            source
                .split_once("\n#[cfg(test)]")
                .map_or(source, |(code, _tests)| code)
                .lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.starts_with("//") && !trimmed.starts_with("--")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        let replace = format!("INSERT {} REPLACE", "OR");
        let upsert = format!("{} CONFLICT", "ON");
        for (name, source) in [("cenovnik", THIS_MODULE), ("catalog", CATALOG)] {
            let shipped = statements(source);
            assert!(
                !shipped.contains(&replace),
                "{name} must not REPLACE a row on the publish path"
            );
            assert!(
                !(shipped.contains("cenovnik_snapshots") && shipped.contains(&upsert)),
                "{name} must not upsert cenovnik_snapshots"
            );
        }

        assert!(
            statements(THIS_MODULE).contains("INSERT INTO cenovnik_snapshots"),
            "and the plain INSERT this test is guarding must actually be here"
        );
    }

    /// `prodajno_mesto` is free text with no foreign key: one typo starts a
    /// second archive lineage and leaves the real outlet with no published
    /// snapshot at all. It is derived from the shop's own settings — the
    /// function takes no outlet parameter for a caller to get wrong.
    #[test]
    fn the_outlet_is_derived_from_settings() {
        with_database("the_outlet_is_derived_from_settings", |db| {
            let connection = db.open().expect("database should open");
            publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z").expect("publish");
            assert_eq!(snapshots(db)[0].prodajno_mesto, OUTLET);
        });
    }

    /// A shop that never filled in the address is still an outlet: its name
    /// identifies it, and `save_company_settings` refuses to leave that empty.
    #[test]
    fn a_shop_without_an_address_is_identified_by_its_name() {
        with_database(
            "a_shop_without_an_address_is_identified_by_its_name",
            |db| {
                set_company(db, "   ", "Butik Ana");
                let connection = db.open().expect("database should open");
                publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("publish");
                assert_eq!(snapshots(db)[0].prodajno_mesto, "Butik Ana");
            },
        );
    }

    /// The archive key is minted once and then frozen. Deriving it afresh on
    /// every publish would let a corrected address typo — or an address filled
    /// in after the fact — start a SECOND lineage: every prior snapshot orphaned
    /// under the old key, the newest-first lookup empty until the next price
    /// write, and the čl. 6 st. 5 comparison of „prethodno objavljenih cena“
    /// with the realtime ones broken across the boundary. The v19 trigger makes
    /// `prodajno_mesto` immutable, so a split lineage can never be re-joined.
    #[test]
    fn correcting_the_address_does_not_start_a_second_archive_lineage() {
        with_database(
            "correcting_the_address_does_not_start_a_second_archive_lineage",
            |db| {
                let connection = db.open().expect("database should open");
                publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("publish");

                set_company(db, "Bulevar oslobođenja 1a, Novi Sad", "Butik Ana");
                let second = publish_current(&connection, &NotConfigured, "2026-08-02T10:15:00Z")
                    .expect("publish")
                    .expect("an identified outlet publishes");

                let archived = snapshots(db);
                assert_eq!(archived.len(), 2, "{archived:?}");
                assert_eq!(
                    archived
                        .iter()
                        .map(|row| row.prodajno_mesto.as_str())
                        .collect::<Vec<_>>(),
                    [OUTLET, OUTLET],
                    "one outlet, one lineage"
                );
                assert_eq!(current_snapshot(db, OUTLET), Some(second));
            },
        );
    }

    /// The v19 CHECK refuses a blank `prodajno_mesto`, and a placeholder lineage
    /// would be a lie about which outlet published what. So an unidentified shop
    /// publishes nothing — and its price save still succeeds, because čl. 6 is
    /// not a reason a till cannot change a price.
    #[test]
    fn an_unidentified_shop_publishes_nothing_and_the_price_save_still_succeeds() {
        with_database(
            "an_unidentified_shop_publishes_nothing_and_the_price_save_still_succeeds",
            |db| {
                set_company(db, "", "");
                let connection = db.open().expect("database should open");
                assert_eq!(
                    publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                        .expect("an unidentified outlet is not an error"),
                    None
                );

                let acting = admin_id(db);
                update_product(db, 1, product_request(31_900), acting)
                    .expect("the price save must not fail for want of an outlet");
                assert!(snapshots(db).is_empty());
            },
        );
    }

    /// The state above, made readable — because a panel that cannot see it
    /// tells the shop a file is written on every price change while nothing is.
    ///
    /// A fresh install sits here: `load_json_setting` hands back
    /// `CompanySettings::default()` without ever persisting it, and
    /// `save_company_settings` is the only writer of the row, so until
    /// Podešavanja → Radnja is saved there is no outlet, no lineage and no
    /// archive — silently, forever.
    #[test]
    fn an_unidentified_shop_reports_no_outlet() {
        with_shop("an_unidentified_shop_reports_no_outlet", |state| {
            set_company(state.db(), "", "");
            assert_eq!(outlet(state).expect("the outlet should read"), None);

            let connection = state.db().open().expect("database should open");
            publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                .expect("an unidentified outlet is not an error");
            assert!(
                snapshots(state.db()).is_empty(),
                "and `None` is exactly the state that archives nothing"
            );
        });
    }

    /// The identified shop reports the key its archive is (and will stay) keyed
    /// on — the frozen one once anything has been published, so the panel and
    /// the archive can never name two different outlets.
    #[test]
    fn an_identified_shop_reports_the_outlet_its_archive_is_keyed_on() {
        with_shop(
            "an_identified_shop_reports_the_outlet_its_archive_is_keyed_on",
            |state| {
                assert_eq!(
                    outlet(state).expect("the outlet should read").as_deref(),
                    Some(OUTLET)
                );

                let connection = state.db().open().expect("database should open");
                publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("publish");
                // The settings move; the frozen key does not, and the panel
                // follows the key rather than today's company name.
                set_company(state.db(), "Knez Mihailova 2, Beograd", "Butik Ana");
                assert_eq!(
                    outlet(state).expect("the outlet should read").as_deref(),
                    Some(OUTLET)
                );
            },
        );
    }

    /// Req. 15 is a founder decision. Until it lands the file is generated and
    /// archived but goes nowhere, and the v19 NULL `published_at` says exactly
    /// that instead of claiming a publication that never happened.
    #[test]
    fn with_no_target_the_snapshot_is_generated_but_not_published() {
        with_database(
            "with_no_target_the_snapshot_is_generated_but_not_published",
            |db| {
                let connection = db.open().expect("database should open");
                publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("a missing target is not an error");

                let archived = snapshots(db);
                assert_eq!(archived.len(), 1, "{archived:?}");
                assert_eq!(archived[0].published_at, None);
                assert_eq!(archived[0].published_target, None);
                assert!(!archived[0].body.is_empty());
            },
        );
    }

    /// What the target receives has to be what the archive keeps, byte for byte
    /// — otherwise the archive answers a different question than „what did the
    /// shop publish“ (čl. 6 st. 5).
    #[test]
    fn a_target_that_accepts_the_body_gets_the_archived_bytes_and_is_recorded() {
        with_database(
            "a_target_that_accepts_the_body_gets_the_archived_bytes_and_is_recorded",
            |db| {
                let connection = db.open().expect("database should open");
                let target = RecordingTarget::new();
                publish_current(&connection, &target, "2026-08-02T09:15:00Z").expect("publish");

                let archived = snapshots(db);
                assert_eq!(archived.len(), 1, "{archived:?}");
                assert_eq!(
                    target.bodies.borrow().as_slice(),
                    &[archived[0].body.clone()]
                );
                assert_eq!(
                    archived[0].published_at.as_deref(),
                    Some("2026-08-02T09:15:00Z")
                );
                assert_eq!(
                    archived[0].published_target.as_deref(),
                    Some(format!("test://{OUTLET}").as_str())
                );
            },
        );
    }

    /// An archived article is not offered, so it has no price to publish — the
    /// same reading `price_history` takes of an inactive product.
    #[test]
    fn only_offered_products_reach_the_published_file() {
        with_database("only_offered_products_reach_the_published_file", |db| {
            let connection = db.open().expect("database should open");
            publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z").expect("publish");

            let archived = snapshots(db);
            assert_eq!(archived[0].row_count, 1);
            assert_eq!(data_rows(&archived[0].body).len(), 1);
            assert!(
                !archived[0].body.contains("ARH-1"),
                "{:?}",
                archived[0].body
            );
        });
    }

    /// Req. 10: the v19 unit-price pair has to reach the file, which means the
    /// publish path has to actually select it. 279,00 RSD for 0,75 l is 372,00
    /// RSD per litar.
    #[test]
    fn the_published_row_carries_the_v19_unit_price_fields() {
        with_database(
            "the_published_row_carries_the_v19_unit_price_fields",
            |db| {
                let connection = db.open().expect("database should open");
                publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                    .expect("publish");

                let body = snapshots(db).remove(0).body;
                let fields: Vec<String> = data_rows(&body)
                    .remove(0)
                    .split(';')
                    .map(str::to_string)
                    .collect();
                assert_eq!(fields[4], "279.00", "{fields:?}");
                assert_eq!(fields[5], "372.00", "{fields:?}");
                assert_eq!(fields[6], "l", "{fields:?}");
            },
        );
    }

    /// `datum_azuriranja` is the date the published price took effect, so it
    /// comes from the offered-price log rather than from `products.updated_at`,
    /// which any edit bumps. Without a logged price it falls back to the row's
    /// own stamp rather than publishing an empty date.
    #[test]
    fn the_published_date_is_when_the_price_last_moved() {
        with_database("the_published_date_is_when_the_price_last_moved", |db| {
            let connection = db.open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     VALUES (1, '2026-07-20T08:00:00Z', 27900, 'update', '2026-07-20T08:00:00Z')",
                    [],
                )
                .expect("price history should insert");
            connection
                .execute(
                    "INSERT INTO products (
                        id, name, sku, category_id, unit_of_measure,
                        sale_price_minor, purchase_price_minor, tax_rate_id,
                        minimum_stock_milli, active, created_at, updated_at
                     )
                     VALUES (3, 'Bez istorije', 'BEZ-1', 1, 'kom', 5000, 3000, 1, 0, 1,
                             '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z')",
                    [],
                )
                .expect("product without history should insert");

            publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z").expect("publish");

            let body = snapshots(db).remove(0).body;
            let dates: Vec<String> = data_rows(&body)
                .iter()
                .map(|row| {
                    row.split(';')
                        .next_back()
                        .expect("datum_azuriranja")
                        .to_string()
                })
                .collect();
            // Rendered in sifra order: BEZ-1 then SOK-075.
            assert_eq!(dates, ["18-06-2026", "20-07-2026"], "{body:?}");
        });
    }

    // ---------------------------------------------------------------------
    // Task 4 — the archive and its retention (req. 14)
    // ---------------------------------------------------------------------

    /// Čl. 6 st. 5 is a duty to enable a comparison of „prethodno objavljenih
    /// cena“ with the realtime ones, so the archive has to read in the order a
    /// person compares in: the file in force first, the ones it replaced behind
    /// it. Two publishes in the same second are the tie v19 breaks by the larger
    /// id — the later insert — and exactly one row may call itself current.
    #[test]
    fn the_archive_lists_the_outlets_snapshots_newest_first_and_names_the_current_one() {
        with_archive("cenovnik_archive_lists_newest_first", |state| {
            let connection = state.db().open().expect("database should open");
            let oldest = publish_current(&connection, &NotConfigured, "2026-08-01T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            let tie_first = publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            let tie_second = publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");

            let listed = list_snapshots(state).expect("the archive should list");
            assert_eq!(
                listed.iter().map(|row| row.id).collect::<Vec<_>>(),
                [tie_second, tie_first, oldest],
                "newest first, and a same-second tie goes to the larger id"
            );
            assert!(listed[0].current, "the newest row is the cenovnik in force");
            assert!(
                listed[1..].iter().all(|row| !row.current),
                "only one file can be the one čl. 6 st. 4 binds the shop to"
            );
            assert!(
                listed.iter().all(|row| row.prodajno_mesto == OUTLET),
                "{listed:?}"
            );
        });
    }

    /// Req. 14: prior publications must remain **retrievable**, not merely
    /// undeleted. What comes back out has to be the bytes the target received —
    /// BOM, CRLF, diacritics and all — or the archive answers a different
    /// question than „what did this shop publish“.
    #[test]
    fn a_snapshot_reads_back_byte_for_byte_what_the_target_received() {
        with_archive("cenovnik_archive_reads_back_byte_for_byte", |state| {
            let connection = state.db().open().expect("database should open");
            let target = RecordingTarget::new();
            publish_current(&connection, &target, "2026-08-01T09:15:00Z").expect("first publish");

            let acting = admin_id(state.db());
            update_product(state.db(), 1, product_request(31_900), acting)
                .expect("price should save");
            publish_current(&connection, &target, "2026-08-03T09:15:00Z").expect("third publish");

            let listed = list_snapshots(state).expect("the archive should list");
            assert_eq!(listed.len(), 3, "{listed:?}");

            let published = target.bodies.borrow().clone();
            let oldest = read_snapshot(state, listed[2].id)
                .expect("the archive should read")
                .expect("the earliest publication is still there");
            assert_eq!(oldest.body, published[0], "byte for byte");
            assert!(oldest.body.starts_with('\u{feff}'), "{:?}", oldest.body);
            assert!(oldest.body.contains("279.00"), "{:?}", oldest.body);
            assert!(!oldest.snapshot.current);

            let newest = read_snapshot(state, listed[0].id)
                .expect("the archive should read")
                .expect("the current publication is there");
            assert_eq!(newest.body, published[1]);
            assert!(newest.body.contains("319.00"), "{:?}", newest.body);
            assert!(newest.snapshot.current);

            assert_eq!(
                read_snapshot(state, 9_999).expect("an unknown id is not an error"),
                None
            );
        });
    }

    /// Čl. 6 st. 2 publishes „posebno za svaki prodajni objekat“, so the
    /// comparison st. 5 asks for runs inside one outlet's lineage. A foreign
    /// outlet's file must never appear in this one's archive, and above all must
    /// never be reachable as this outlet's current cenovnik.
    #[test]
    fn the_archive_lists_only_the_outlets_own_lineage() {
        with_archive("cenovnik_archive_is_per_outlet", |state| {
            let connection = state.db().open().expect("database should open");
            publish_current(&connection, &NotConfigured, "2026-08-01T09:15:00Z").expect("publish");
            connection
                .execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('Druga radnja', '2026-08-05T09:15:00Z', 1, 'h-druga',
                             'telo druge radnje', '2026-08-05T09:15:00Z')",
                    [],
                )
                .expect("a second outlet should insert");

            let listed = list_snapshots(state).expect("the archive should list");
            assert_eq!(listed.len(), 1, "{listed:?}");
            assert_eq!(listed[0].prodajno_mesto, OUTLET);
            assert!(listed[0].current);
        });
    }

    /// Req. 14's floor is the ZZP čl. 213 two-year limitation, and it is read out
    /// of the shared `retention_policies` table rather than hard-coded at the
    /// cut. A file older than that answers nothing anybody may still ask; a
    /// younger one is inside the window a proceeding could still open in.
    #[test]
    fn a_snapshot_past_the_cl_213_limitation_is_purged_and_a_younger_one_is_kept() {
        with_archive("cenovnik_archive_purges_past_the_limitation", |state| {
            let connection = state.db().open().expect("database should open");
            let expired = publish_current(&connection, &NotConfigured, "2024-01-05T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            let inside = publish_current(&connection, &NotConfigured, "2025-06-01T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            let current = publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            drop(connection);

            // Two years back from this day is 2024-08-02: the first file is
            // behind it, the second is not.
            let removed = purge_expired_snapshots(state, "2026-08-02T09:15:00Z")
                .expect("the purge should run");
            assert_eq!(removed, 1, "only the file past the limitation goes");

            let listed = list_snapshots(state).expect("the archive should list");
            assert_eq!(
                listed.iter().map(|row| row.id).collect::<Vec<_>>(),
                [current, inside],
                "{listed:?}"
            );
            assert_eq!(
                read_snapshot(state, expired).expect("the archive should read"),
                None
            );
        });
    }

    /// The outlet's current cenovnik is the file čl. 6 st. 4 binds the shop to
    /// today, and čl. 6 st. 5's comparison has nothing to compare against once it
    /// is gone. A shop that has not moved a price in three years must still have
    /// its published file — so the retention floor may never reach the newest row
    /// for an outlet, whatever its date.
    #[test]
    fn the_purge_never_removes_an_outlets_current_cenovnik_however_old_it_is() {
        with_archive("cenovnik_archive_keeps_the_current_file", |state| {
            let connection = state.db().open().expect("database should open");
            let only = publish_current(&connection, &NotConfigured, "2024-01-05T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            connection
                .execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('Druga radnja', '2024-01-05T09:15:00Z', 1, 'h-druga',
                             'telo druge radnje', '2024-01-05T09:15:00Z')",
                    [],
                )
                .expect("a second outlet should insert");
            drop(connection);

            assert_eq!(
                purge_expired_snapshots(state, "2030-01-01T09:15:00Z")
                    .expect("the purge should run"),
                0,
                "every outlet keeps the file it is currently bound to"
            );

            let listed = list_snapshots(state).expect("the archive should list");
            assert_eq!(listed.len(), 1, "{listed:?}");
            assert_eq!(listed[0].id, only);
            assert!(listed[0].current);

            let survivors: i64 = state
                .db()
                .open()
                .expect("database should open")
                .query_row("SELECT COUNT(*) FROM cenovnik_snapshots", [], |row| {
                    row.get(0)
                })
                .expect("the archive should count");
            assert_eq!(survivors, 2, "the second outlet keeps its current file too");
        });
    }

    /// „Retention resolves through the shared table“ is the requirement, not
    /// „two years are hard-coded at the cut“. A legal hold on the class stops
    /// this purge the way it stops every other, and a rok the shop moved forward
    /// postpones it — both without a line changing in this module.
    #[test]
    fn the_purge_obeys_the_shared_retention_row() {
        with_archive("cenovnik_archive_obeys_the_shared_row", |state| {
            let connection = state.db().open().expect("database should open");
            publish_current(&connection, &NotConfigured, "2024-01-05T09:15:00Z").expect("publish");
            publish_current(&connection, &NotConfigured, "2026-08-02T09:15:00Z").expect("publish");

            connection
                .execute(
                    "UPDATE retention_policies SET legal_hold = 1 WHERE record_class = ?1",
                    params![RecordClass::CenovnikArchive.key()],
                )
                .expect("a legal hold should store");
            assert_eq!(
                purge_expired_snapshots(state, "2026-08-02T09:15:00Z")
                    .expect("a held class is not an error"),
                0,
                "a legal hold outranks the limitation"
            );

            connection
                .execute(
                    "UPDATE retention_policies SET legal_hold = 0 WHERE record_class = ?1",
                    params![RecordClass::CenovnikArchive.key()],
                )
                .expect("the hold should lift");
            extend_retain_until(
                &connection,
                RecordClass::CenovnikArchive,
                "2027-01-01",
                "2026-08-02T09:15:00Z",
            )
            .expect("the shop should be able to keep the archive longer");
            assert_eq!(
                purge_expired_snapshots(state, "2026-08-02T09:15:00Z")
                    .expect("an unreached floor is not an error"),
                0,
                "a rok the shop moved forward postpones the cut"
            );
            drop(connection);

            assert_eq!(
                purge_expired_snapshots(state, "2027-01-01T09:15:00Z")
                    .expect("the purge should run"),
                1,
                "and once the moved rok is reached the expired file goes"
            );
            assert_eq!(CENOVNIK_ARCHIVE_RETENTION_YEARS, 2);
        });
    }

    // ---------------------------------------------------------------------
    // Task 5 — what the till guard compares against (req. 12)
    // ---------------------------------------------------------------------

    /// The guard measures a rung price against the file the shop actually
    /// published, not against the catalog that file was rendered from — the
    /// catalog is the very thing that may have moved without a republish. So the
    /// lookup returns the outlet's current snapshot, its identity, and the prices
    /// read back out of its body.
    #[test]
    fn the_current_published_cenovnik_is_the_outlets_newest_file_and_its_prices() {
        with_database("cenovnik_current_published_prices", |db| {
            let connection = db.open().expect("database should open");
            publish_current(&connection, &NotConfigured, "2026-08-01T09:15:00Z").expect("publish");

            let acting = admin_id(db);
            update_product(db, 1, product_request(31_900), acting).expect("price should save");

            let published = current_published_cenovnik(&connection)
                .expect("the archive should read")
                .expect("a shop that has published has a current cenovnik");
            let archived = snapshots(db);
            assert_eq!(archived.len(), 2, "{archived:?}");
            assert_eq!(published.snapshot_id, archived[1].id, "the newest file");
            assert_eq!(published.generated_at, archived[1].generated_at);
            assert_eq!(published.content_hash, archived[1].content_hash);
            assert_eq!(published.prodajna_cena("SOK-075"), Some(31_900));
            assert_eq!(
                published.prodajna_cena("ARH-1"),
                None,
                "an article that is not offered has no published price"
            );
        });
    }

    /// „No published snapshot means no guard.“ A shop that has published nothing
    /// — or has not identified its prodajni objekat yet — must still be able to
    /// sell, so the lookup answers `None` rather than erroring or inventing a
    /// baseline.
    #[test]
    fn a_shop_that_has_published_nothing_has_no_current_cenovnik() {
        with_database("cenovnik_current_published_none", |db| {
            let connection = db.open().expect("database should open");
            assert_eq!(
                current_published_cenovnik(&connection).expect("an empty archive is not an error"),
                None
            );

            set_company(db, "", "");
            assert_eq!(
                current_published_cenovnik(&connection)
                    .expect("an unidentified outlet is not an error"),
                None,
                "with no outlet there is no lineage to read"
            );
        });
    }

    /// Čl. 6 st. 2 publishes „posebno za svaki prodajni objekat“, so a foreign
    /// outlet's newer file must never become the price this till is measured
    /// against.
    #[test]
    fn another_outlets_newer_file_is_never_this_tills_published_price() {
        with_database("cenovnik_current_published_per_outlet", |db| {
            let connection = db.open().expect("database should open");
            let ours = publish_current(&connection, &NotConfigured, "2026-08-01T09:15:00Z")
                .expect("publish")
                .expect("an identified outlet publishes");
            connection
                .execute(
                    "INSERT INTO cenovnik_snapshots (prodajno_mesto, generated_at, row_count,
                                                     content_hash, body, created_at)
                     VALUES ('Druga radnja', '2026-08-05T09:15:00Z', 1, 'h-druga',
                             ?1, '2026-08-05T09:15:00Z')",
                    params!["\u{feff}sifra;prodajna_cena\r\nSOK-075;1.00\r\n"],
                )
                .expect("a second outlet should insert");

            let published = current_published_cenovnik(&connection)
                .expect("the archive should read")
                .expect("our outlet has a current cenovnik");
            assert_eq!(published.snapshot_id, ours);
            assert_eq!(published.prodajna_cena("SOK-075"), Some(27_900));
        });
    }

    /// v19 leaves `DELETE` open on purpose — čl. 213 gives the archive a
    /// two-year limitation rather than a `trajno` duty, so the purge has to be
    /// able to reach an expired snapshot, and no trigger can tell a purge from a
    /// cover-up. The constraint therefore lives in the code: this module's
    /// retention purge is the only thing allowed to delete a published cenovnik.
    ///
    /// The needle is composed at run time so this assertion cannot match its own
    /// source text, and only shipped statements are scanned — every comment line
    /// and every `#[cfg(test)] mod` is dropped first, so the prose explaining the
    /// rule cannot trip it. Every spelling SQLite accepts is one statement to
    /// [`deletes_the_archive_table`]: the crate writing its SQL uppercase and
    /// unqualified is a house style, and a guard that leans on a house style is
    /// a guard a lowercase `delete` walks past.
    #[test]
    fn the_retention_purge_is_the_only_code_that_deletes_a_published_cenovnik() {
        let sources = shipped_sources();
        assert!(
            sources.len() > 20,
            "the scan must actually see the crate: {}",
            sources.len()
        );

        let offenders: Vec<&str> = sources
            .iter()
            .filter(|(path, source)| {
                path != "commands/cenovnik.rs" && deletes_the_archive_table(source)
            })
            .map(|(path, _)| path.as_str())
            .collect();
        assert!(
            offenders.is_empty(),
            "only the retention purge in commands/cenovnik.rs may delete a published cenovnik, \
             and these modules do too: {offenders:?}"
        );

        let purge = sources
            .iter()
            .find(|(path, _)| path == "commands/cenovnik.rs")
            .map(|(_, source)| source.as_str())
            .expect("this module must be in the scan");
        assert!(
            deletes_the_archive_table(purge),
            "and the purge this test is guarding must actually be here"
        );
    }

    /// What the scan above has to recognise, and what it must leave alone.
    ///
    /// The guard is only worth the claim its doc comment makes if it sees every
    /// statement SQLite would execute, not the one spelling this crate happens
    /// to write today. `delete from cenovnik_snapshots` and
    /// `DELETE FROM main.cenovnik_snapshots` are both valid, both really remove
    /// a published cenovnik, and an exact-literal match let both through — the
    /// crate writing its SQL uppercase and unqualified is a convention, not a
    /// property, and a guard that depends on a convention guards nothing.
    ///
    /// The near-misses matter as much: a differently named table is not this
    /// table, and reading `cenovnik_snapshots` is not deleting from it.
    #[test]
    fn the_delete_scan_reads_sql_the_way_sqlite_does() {
        for statement in [
            "DELETE FROM cenovnik_snapshots WHERE id = ?1",
            "delete from cenovnik_snapshots",
            "Delete From Cenovnik_Snapshots",
            "DELETE FROM main.cenovnik_snapshots",
            "DELETE FROM \"cenovnik_snapshots\"",
            "DELETE FROM \"main\".\"cenovnik_snapshots\";",
            "DELETE FROM `cenovnik_snapshots`",
            "DELETE FROM [cenovnik_snapshots]",
        ] {
            assert!(
                deletes_the_archive_table(statement),
                "this really deletes a published cenovnik and the guard must see it: {statement}"
            );
        }

        for statement in [
            "SELECT id FROM cenovnik_snapshots WHERE prodajno_mesto = ?1",
            "INSERT INTO cenovnik_snapshots (prodajno_mesto) VALUES (?1)",
            "DELETE FROM cenovnik_snapshots_backup",
            "DELETE FROM stavke_cenovnik_snapshots",
            "DELETE FROM kalkulacije",
        ] {
            assert!(
                !deletes_the_archive_table(statement),
                "this leaves the archive alone and the guard must not name it: {statement}"
            );
        }
    }

    /// Does this shipped source remove rows from the archive table?
    ///
    /// Read the way SQLite reads it, not the way this crate happens to write it.
    /// Keywords are case-folded, and the table may carry a schema qualifier or
    /// any of SQLite's three quotings — each is a valid spelling of the same
    /// deletion, and matching one exact literal let every other one through.
    /// Only the last dot-separated segment is compared, and it is compared whole,
    /// so `stavke_cenovnik_snapshots` and `cenovnik_snapshots_backup` stay other
    /// tables.
    ///
    /// The needle is composed at run time so the guard cannot match its own
    /// source text, which is why the two halves of `DELETE FROM` are joined here
    /// rather than written out.
    fn deletes_the_archive_table(source: &str) -> bool {
        const TABLE: &str = "CENOVNIK_SNAPSHOTS";
        let opener = format!("{} {} ", "DELETE", "FROM");
        let folded = source.to_ascii_uppercase();

        folded.match_indices(&opener).any(|(index, _)| {
            let target = folded[index + opener.len()..]
                .split_whitespace()
                .next()
                .unwrap_or_default();
            // `"x"`, `` `x` ``, `[x]`, a trailing `;` and the closing `"` of the
            // Rust literal all fall away; the dots survive so the qualifier can
            // be dropped deliberately rather than by accident.
            let bare: String = target
                .chars()
                .filter(|character| {
                    character.is_ascii_alphanumeric() || *character == '_' || *character == '.'
                })
                .collect();
            bare.rsplit('.').next() == Some(TABLE)
        })
    }

    /// Every `.rs` file under `src/`, reduced to the statements the app ships:
    /// the `#[cfg(test)] mod …` tail removed, every Rust and SQL comment line
    /// dropped, and whitespace collapsed so a statement wrapped across lines
    /// still reads as one.
    fn shipped_sources() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        collect_rust_files(&root, &root, &mut found);
        found.sort();
        found
    }

    fn collect_rust_files(
        root: &std::path::Path,
        directory: &std::path::Path,
        found: &mut Vec<(String, String)>,
    ) {
        let entries = std::fs::read_dir(directory).expect("the crate source should be readable");
        for entry in entries {
            let path = entry.expect("a directory entry should read").path();
            if path.is_dir() {
                collect_rust_files(root, &path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(root)
                    .expect("every file is under the source root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let source = std::fs::read_to_string(&path).expect("a source file should read");
                found.push((relative, shipped_statements(&source)));
            }
        }
    }

    // ---------------------------------------------------------------------
    // Task 7 — where the file goes (reqs. 13, 15)
    // ---------------------------------------------------------------------

    /// Req. 15 leaves the hosting question to the founder, so where the file
    /// goes is the shop's own setting — and the shop owner's alone. A cashier
    /// able to move it could point the published prices at a folder nobody
    /// serves, and čl. 6 st. 3 would be breached by a screen the owner never
    /// opened.
    #[test]
    fn only_an_admin_can_choose_where_the_cenovnik_is_published() {
        with_shop("cenovnik_publish_target_is_admin_only", |state| {
            with_publish_folder("cenovnik-target-admin-only", |folder| {
                let configured = PublishTargetSettings::LocalFolder {
                    folder: folder.display().to_string(),
                };

                sign_in_cashier(state);
                let error = save_publish_target(state, configured.clone())
                    .expect_err("a cashier may not move the shop's published prices");
                assert_eq!(error.code(), "forbidden");
                assert_eq!(
                    publish_target(state).expect("the setting should read"),
                    PublishTargetSettings::NotConfigured,
                    "the refused write must not have landed"
                );

                sign_in_admin(state);
                assert_eq!(
                    save_publish_target(state, configured.clone())
                        .expect("the owner configures the target"),
                    configured
                );
                assert_eq!(
                    publish_target(state).expect("the setting should read"),
                    configured
                );
            });
        });
    }

    /// The default is the honest one: nothing is configured, and nothing claims
    /// otherwise. A shop that has never opened the panel is not misdescribed as
    /// publishing somewhere.
    #[test]
    fn nothing_is_configured_until_the_shop_says_so() {
        with_shop("cenovnik_publish_target_defaults_to_none", |state| {
            assert_eq!(
                publish_target(state).expect("an unset target is not an error"),
                PublishTargetSettings::NotConfigured
            );
        });
    }

    /// Req. 11 through req. 15: a price write republishes, and with a local
    /// folder configured the file actually leaves the database. What lands on
    /// disk is the archived body byte for byte — čl. 6 st. 4 binds the shop to
    /// what a fetch of that file returns, and the archive is the evidence of
    /// what it published.
    #[test]
    fn a_price_write_publishes_into_the_configured_folder() {
        with_shop("cenovnik_publishes_into_the_configured_folder", |state| {
            with_publish_folder("cenovnik-target-price-write", |folder| {
                sign_in_admin(state);
                save_publish_target(
                    state,
                    PublishTargetSettings::LocalFolder {
                        folder: folder.display().to_string(),
                    },
                )
                .expect("the owner configures the target");

                let acting = admin_id(state.db());
                update_product(state.db(), 1, product_request(31_900), acting)
                    .expect("price should save");

                let archived = snapshots(state.db());
                assert_eq!(archived.len(), 1, "{archived:?}");

                let path = folder.join(published_file_name(OUTLET));
                let written = std::fs::read(&path).expect("the published file should read");
                assert_eq!(
                    written.as_slice(),
                    archived[0].body.as_bytes(),
                    "the file the shop serves is the file the archive kept"
                );
                assert!(
                    archived[0].body.contains("319.00"),
                    "{:?}",
                    archived[0].body
                );
                assert!(
                    archived[0].published_at.is_some(),
                    "a snapshot a target accepted must say so: {archived:?}"
                );
                assert_eq!(
                    archived[0].published_target.as_deref(),
                    Some(path.display().to_string().as_str()),
                    "and must name where it went"
                );
            });
        });
    }

    /// The republish path is where Task 7's founder decision lands, so the
    /// resolution of the configured target has to happen there rather than at
    /// each of the four write paths. With nothing configured the file is still
    /// generated and archived — it simply goes nowhere, which is what a NULL
    /// `published_at` says.
    #[test]
    fn with_nothing_configured_the_republish_archives_and_publishes_nowhere() {
        with_shop("cenovnik_republish_without_a_target", |state| {
            let connection = state.db().open().expect("database should open");
            republish_after_price_move(&connection, "2026-08-02T09:15:00Z");

            let archived = snapshots(state.db());
            assert_eq!(archived.len(), 1, "{archived:?}");
            assert_eq!(archived[0].published_at, None);
            assert_eq!(archived[0].published_target, None);
            assert!(!archived[0].body.is_empty());
        });
    }

    /// A configured folder that cannot be written is the routine failure — an
    /// unmounted drive, a folder the shop deleted, a name that is really a
    /// file. The publish runs after the price write has committed, so it may
    /// cost neither the price nor the archived snapshot; the file simply is not
    /// published, and the NULL `published_at` is what Task 8's panel reads to
    /// tell the operator the published copy is stale.
    #[test]
    fn a_target_that_cannot_be_written_costs_neither_the_price_nor_the_snapshot() {
        with_shop("cenovnik_target_that_cannot_be_written", |state| {
            with_publish_folder("cenovnik-target-unwritable", |folder| {
                let occupied = folder.join("zauzeto");
                std::fs::write(&occupied, b"ovo nije folder").expect("the blocker should write");

                sign_in_admin(state);
                save_publish_target(
                    state,
                    PublishTargetSettings::LocalFolder {
                        folder: occupied.display().to_string(),
                    },
                )
                .expect("the owner may name a folder that later stops working");

                let acting = admin_id(state.db());
                update_product(state.db(), 1, product_request(31_900), acting)
                    .expect("a committed price save must not be reported as failed");

                let archived = snapshots(state.db());
                assert_eq!(archived.len(), 1, "the archive still has the file");
                assert_eq!(
                    archived[0].published_at, None,
                    "and must not claim a publication that failed"
                );
                assert_eq!(archived[0].published_target, None);
                let saved: i64 = state
                    .db()
                    .open()
                    .expect("database should open")
                    .query_row(
                        "SELECT sale_price_minor FROM products WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .expect("price should read");
                assert_eq!(saved, 31_900);
            });
        });
    }

    /// A settings row this cannot parse must not take a price save down with
    /// it. It lands in „nothing is configured“ — the same honest state as an
    /// unconfigured shop — rather than erroring on the till.
    #[test]
    fn an_unreadable_target_setting_publishes_nowhere_instead_of_failing() {
        with_shop("cenovnik_target_setting_is_corrupt", |state| {
            let connection = state.db().open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO settings (key, value_json, updated_at)
                     VALUES ('cenovnik_publish_target', 'ovo nije JSON', '2026-08-02T09:15:00Z')",
                    [],
                )
                .expect("a corrupt setting should insert");

            assert_eq!(
                publish_target(state).expect("a corrupt setting is not an error"),
                PublishTargetSettings::NotConfigured
            );

            let acting = admin_id(state.db());
            update_product(state.db(), 1, product_request(31_900), acting)
                .expect("the price save must not fail for want of a readable target");
            let archived = snapshots(state.db());
            assert_eq!(archived.len(), 1, "{archived:?}");
            assert_eq!(archived[0].published_at, None);
        });
    }

    /// The folder is typed by hand into a settings field. An empty one is not a
    /// target, and a relative one resolves against whatever directory the app
    /// happened to be launched from — so the shop would be told its cenovnik is
    /// published and be unable to say where.
    #[test]
    fn the_folder_must_be_named_and_must_be_an_absolute_path() {
        with_shop("cenovnik_target_folder_is_validated", |state| {
            sign_in_admin(state);

            for folder in ["", "   "] {
                let error = save_publish_target(
                    state,
                    PublishTargetSettings::LocalFolder {
                        folder: folder.to_string(),
                    },
                )
                .expect_err("an empty folder is not a target");
                assert_eq!(error.code(), "validation_error");
            }

            let error = save_publish_target(
                state,
                PublishTargetSettings::LocalFolder {
                    folder: "cenovnik/javno".to_string(),
                },
            )
            .expect_err("a relative folder is not somewhere the shop can point at");
            assert_eq!(error.code(), "validation_error");

            assert_eq!(
                publish_target(state).expect("the setting should read"),
                PublishTargetSettings::NotConfigured,
                "no refused folder may have landed"
            );
        });
    }

    /// Turning publication back off has to be possible: a shop that moves its
    /// site, or decides to upload by hand for a while, must be able to say so —
    /// and the file must then stop leaving the machine.
    #[test]
    fn the_target_can_be_set_back_to_nothing_configured() {
        with_shop("cenovnik_target_can_be_cleared", |state| {
            with_publish_folder("cenovnik-target-cleared", |folder| {
                sign_in_admin(state);
                save_publish_target(
                    state,
                    PublishTargetSettings::LocalFolder {
                        folder: folder.display().to_string(),
                    },
                )
                .expect("the owner configures the target");
                save_publish_target(state, PublishTargetSettings::NotConfigured)
                    .expect("the owner turns publication back off");

                let connection = state.db().open().expect("database should open");
                republish_after_price_move(&connection, "2026-08-02T09:15:00Z");

                let archived = snapshots(state.db());
                assert_eq!(archived.len(), 1, "{archived:?}");
                assert_eq!(archived[0].published_at, None);
                assert_eq!(
                    std::fs::read_dir(folder)
                        .expect("the folder should read")
                        .count(),
                    0,
                    "nothing leaves the machine once the target is cleared"
                );
            });
        });
    }

    /// Task 8's panel prints a fine figure, and req. 18 says which one. The
    /// panel may not author it — every statutory sum lives in `legal.rs` — so it
    /// is resolved here against the **stored** legal form and handed over whole.
    ///
    /// An unset form yields no figure at all: the panel then says nothing about
    /// money rather than guessing a tier, which is the one mistake that cannot be
    /// walked back once a shop owner has read it.
    #[test]
    fn the_notice_resolves_the_stored_tier_and_stays_silent_while_it_is_unset() {
        with_shop("cenovnik_notice_tier", |state| {
            sign_in_admin(state);

            let unset = cenovnik_notice(state).expect("notice should resolve");
            assert!(
                unset.penalty.is_none(),
                "no legal form is stored yet, so no figure may be quoted: {:?}",
                unset.penalty
            );
            assert!(
                unset.citation.contains("čl. 6 st. 1–3"),
                "{}",
                unset.citation
            );

            crate::commands::settings::save_shop_profile(
                state,
                crate::commands::settings::ShopProfileRequest {
                    pravna_forma: Some(crate::commands::settings::PravnaForma::Preduzetnik),
                    pdv_obveznik: Some(false),
                    distance_selling: Some(false),
                    lpfr_in_premises: Some(true),
                    lpfr_carve_out_internet_only: Some(false),
                    lpfr_carve_out_own_used_assets: Some(false),
                    esir_elements: Vec::new(),
                },
            )
            .expect("admin should save the profile");

            let notice = cenovnik_notice(state).expect("notice should resolve");
            let penalty = notice.penalty.expect("the stored tier is known");
            assert!(
                penalty.contains("100.000"),
                "the preduzetnik row is ZZP čl. 210 st. 3: {penalty}"
            );
            assert!(
                penalty.contains("50.000"),
                "ZoP čl. 173 st. 1 halves it on payment within eight days: {penalty}"
            );
            assert!(
                !penalty.contains("200.000"),
                "the pravno-lice sum must never reach a preduzetnik: {penalty}"
            );
            assert!(
                notice.summary.contains("nije razjašnjeno"),
                "the no-website case is unresolved and the panel says so: {}",
                notice.summary
            );
        });
    }

    fn shipped_statements(source: &str) -> String {
        let lines: Vec<&str> = source.lines().collect();
        // The tests module opens with `#[cfg(test)]` followed by a `mod … {`.
        // A `#[cfg(test)]` on a single helper item, or on a `mod x;`
        // declaration, is not the boundary and must not truncate the scan.
        let end = (0..lines.len())
            .find(|&index| {
                lines[index].trim() == "#[cfg(test)]"
                    && lines.get(index + 1).is_some_and(|next| {
                        next.trim_start().starts_with("mod ") && next.trim_end().ends_with('{')
                    })
            })
            .unwrap_or(lines.len());

        lines[..end]
            .iter()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("--")
            })
            .flat_map(|line| line.split_whitespace())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
