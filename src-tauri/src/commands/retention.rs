//! The rok čuvanja an operator may actually move (SW-10 req. 6, SW-13 req. 22).
//!
//! **Why this module exists at all.** Req. 6 asks for *configurable retention
//! with a documented default* and req. 22 for a default that is shipped, a
//! setting that is exposed, and the chosen value recorded in the čl. 47
//! register. `retention::extend_retain_until` shipped the arithmetic and the
//! upward-only rule, but nothing outside `#[cfg(test)]` ever called it — while
//! four strings (two rows of the čl. 23 notice handed to the employee, the
//! `napomena` stored beside the access-log class, and the `rok_osnov` the čl. 47
//! register prints for the Poverenik) told the reader the period was adjustable.
//! A promise with no verb behind it is the same defect as a purge no job
//! performs, one document further on.
//!
//! **[`AdjustableClass`] is the boundary, and it is a type.** Like
//! `personnel::PurgeableClass`, it names the classes this command can reach and
//! nothing else — the ZEOR čl. 5 evidencija, the frozen monthly classification
//! and the čl. 47 register have no variant, so reaching them is a compile error
//! rather than something a reviewer has to catch. `trajno` is not a long period;
//! it is the absence of an end, and every concrete date is shorter than it, so a
//! „podešavanje“ that could write one would only ever shorten.
//!
//! **Upward only, and refused rather than clamped.** A silent `max()` would
//! satisfy the same invariant while hiding the caller that tried to shorten the
//! period, and the shop would learn about it from an inspector. The refusal
//! comes back as prose the operator can act on.
//!
//! **The chosen value has to reach the register.** Čl. 47 st. 1 t. 6 is a period
//! *per vrsta podataka*, and [`crate::cl47`] reads it out of `retention_policies`
//! at generation time — so this command regenerates the register in the same
//! breath rather than leaving the shop's own statement to the Poverenik carrying
//! the seeded default until the next launch.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::audit::{AuditAction, AuditDraft, AuditObjectType};
use crate::clock::utc_now;
use crate::commands::audit::append_audit_event;
use crate::commands::auth::require_admin;
use crate::retention::{extend_retain_until, RecordClass};
use crate::state::AppState;

/// The classes whose rok a registered command can move — the whole list.
///
/// Every variant resolves to a [`RecordClass`] whose `never_purge` is false,
/// pinned by `every_class_that_is_not_trajno_is_named_by_an_adjustable_variant`,
/// which also demands the converse: a bounded class with no variant here is a
/// class whose „podesiv“ prose nothing keeps, and `docs_guard` reads this list to
/// decide which claims the documents are allowed to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustableClass {
    /// The standalone čl. 55 st. 6 overtime register — three years by default
    /// (`STANDALONE_OVERTIME_LOG_FLOOR_YEARS`), and no statute sets a period.
    WorktimeOvertimeLog,
    /// Entry drafts and advisory clock data. The period close is what gates
    /// this class; the date is the shared row's half of the same decision.
    WorktimeDraft,
    /// The PIN and password hashes. The termination event is what discards
    /// them (req. 21) — moving this date postpones the sweep, it does not time
    /// it.
    Credentials,
    /// The čl. 48 evidencija pristupa — `ACCESS_LOG_RETENTION_YEARS` by default,
    /// and the class req. 22 is written about.
    AccessLog,
    /// The archive of published cenovnici (SW-12 req. 14) —
    /// `CENOVNIK_ARCHIVE_RETENTION_YEARS` by default, on ZZP čl. 213's
    /// two-year limitation. Nothing in the ZZP prescribes a period, so a shop
    /// that wants a longer published-price record may keep one.
    CenovnikArchive,
    /// The popisne liste and the rest of the popis documentation (SW-16 req. 42)
    /// — [`crate::retention::POPIS_RETENTION_YEARS`] on ZoRač čl. 28 st. 7,
    /// counted from the last day of the business year (st. 9). No provision
    /// names popisne liste expressly (§6 R-7), so the five years are an
    /// inference and a shop whose knjigovođa reads it longer must be able to say
    /// so — which is the whole reason this class is here rather than fixed.
    PopisDokumentacija,
}

impl AdjustableClass {
    pub const ALL: [Self; 6] = [
        Self::WorktimeOvertimeLog,
        Self::WorktimeDraft,
        Self::Credentials,
        Self::AccessLog,
        Self::CenovnikArchive,
        Self::PopisDokumentacija,
    ];

    /// The shared `retention_policies` row this variant moves.
    pub fn record_class(self) -> RecordClass {
        match self {
            Self::WorktimeOvertimeLog => RecordClass::WorktimeOvertimeLog,
            Self::WorktimeDraft => RecordClass::WorktimeDraft,
            Self::Credentials => RecordClass::Credentials,
            Self::AccessLog => RecordClass::AccessLog,
            Self::CenovnikArchive => RecordClass::CenovnikArchive,
            Self::PopisDokumentacija => RecordClass::PopisDokumentacija,
        }
    }

    /// The stored `record_class` string, inverted. `None` for a `trajno` class
    /// and for anything that is not a class at all — the two are told apart by
    /// [`extend_policy`], which owes the operator different sentences.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|class| class.record_class().key() == key)
    }
}

/// One policy row as the operator's screen needs it: the rok in force, the class
/// it belongs to, and whether anything in this program can move it.
///
/// `adjustable` is derived from [`AdjustableClass`] rather than from
/// `never_purge`, so the flag on screen means *a command can move this* and not
/// *the table says it is not trajno*. Those two can only ever disagree by
/// mistake, and this is the side that has to be true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicyView {
    pub record_class: RecordClass,
    /// What the class is called in front of a person.
    pub naziv: &'static str,
    /// The earliest `gggg-MM-dd` on which the class may be discarded. `None` is
    /// `trajno` — the absence of an end, never „no rule“.
    pub retain_until: Option<String>,
    pub legal_hold: bool,
    pub never_purge: bool,
    pub adjustable: bool,
    /// The stored note, verbatim — the sentence a person reads when they ask why
    /// a record is still there.
    pub napomena: &'static str,
    pub updated_at: String,
}

/// The whole table, `trajno` classes included.
///
/// Admin-gated inside the domain function, not in the `#[tauri::command]`
/// wrapper, so no in-process caller can route around it. The trajno rows are
/// listed on purpose: „ova evidencija se čuva trajno i rok se ne podešava“ is
/// the answer to a question the operator would otherwise ask by trying.
pub fn list_policies(state: &AppState) -> Result<Vec<RetentionPolicyView>, AppError> {
    require_admin(state)?;

    let connection = state.db().open()?;
    RecordClass::ALL
        .into_iter()
        .map(|class| read_view(&connection, class))
        .collect()
}

/// Moves one class's rok forward.
///
/// Three refusals, and each one alone is enough: an unknown class, a class no
/// command may move, and a date earlier than the stored one (the last is
/// [`extend_retain_until`]'s, which also rejects a rok that is not `gggg-MM-dd`
/// and a class stored without a floor).
///
/// The write and its čl. 48 line share one transaction. The register is
/// regenerated after the commit, because [`crate::cl47::generate`] opens its own
/// connection — a stale register would tell the Poverenik a period the till no
/// longer applies, which is the one thing req. 28 says the generated document
/// exists to prevent.
pub fn extend_policy(
    state: &AppState,
    record_class: &str,
    retain_until: &str,
    now: &str,
) -> Result<RetentionPolicyView, AppError> {
    let acting = require_admin(state)?;

    let Some(class) = RecordClass::from_key(record_class) else {
        return Err(AppError::validation(
            format!("Klasa čuvanja „{record_class}“ ne postoji u tabeli rokova."),
            serde_json::json!({ "recordClass": record_class }),
        ));
    };

    let Some(adjustable) = AdjustableClass::from_key(record_class) else {
        let razlog = if class.never_purge() {
            "ova evidencija se čuva trajno, a trajno je odsustvo roka — svaki datum bi ga skratio"
        } else {
            "za ovu klasu program ne nudi izmenu roka"
        };
        return Err(AppError::validation(
            format!("Rok za „{}“ se ne podešava: {razlog}.", class.naziv()),
            serde_json::json!({
                "recordClass": record_class,
                "neverPurge": class.never_purge(),
            }),
        ));
    };

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;
    extend_retain_until(&tx, adjustable.record_class(), retain_until, now)?;

    // Req. 4: the row's own id and nothing else. The class key would pass the
    // whitelist, but `audit_events` carries surrogate ids everywhere else and a
    // second convention in one column is how a value payload eventually arrives.
    let policy_id: i64 = tx.query_row(
        "SELECT id FROM retention_policies WHERE record_class = ?1",
        params![class.key()],
        |row| row.get(0),
    )?;
    append_audit_event(
        &tx,
        &AuditDraft {
            at: now.to_string(),
            actor_user_id: Some(acting.id),
            action: AuditAction::Menjanje,
            object_type: AuditObjectType::RetentionPolicy,
            object_id: policy_id.to_string(),
            reason_code: None,
            recipient: None,
            support_session_id: None,
        },
    )?;

    let view = read_view(&tx, class)?;
    tx.commit()?;

    crate::cl47::generate(state, now)?;

    Ok(view)
}

/// One row, read the way `retention::load_policy` reads it: a missing row is an
/// error and never a default, because a table with no policy is exactly the
/// state req. 42 exists to prevent.
fn read_view(conn: &Connection, class: RecordClass) -> Result<RetentionPolicyView, AppError> {
    conn.query_row(
        "SELECT retain_until, legal_hold, never_purge, updated_at
         FROM retention_policies
         WHERE record_class = ?1",
        params![class.key()],
        |row| {
            Ok(RetentionPolicyView {
                record_class: class,
                naziv: class.naziv(),
                retain_until: row.get(0)?,
                legal_hold: row.get::<_, i64>(1)? != 0,
                never_purge: row.get::<_, i64>(2)? != 0,
                adjustable: AdjustableClass::from_key(class.key()).is_some(),
                napomena: class.napomena(),
                updated_at: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| {
        AppError::not_found(format!(
            "Klasa čuvanja „{}“ nije upisana u tabelu rokova.",
            class.key()
        ))
    })
}

#[tauri::command]
pub fn retention_list_policies(
    state: State<'_, AppState>,
) -> Result<Vec<RetentionPolicyView>, CommandError> {
    list_policies(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn retention_extend_policy(
    state: State<'_, AppState>,
    record_class: String,
    retain_until: String,
) -> Result<RetentionPolicyView, CommandError> {
    extend_policy(state.inner(), &record_class, &retain_until, &utc_now()?).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{extend_policy, list_policies, AdjustableClass};
    use crate::cl47;
    use crate::db::{test_database_path, Db};
    use crate::retention::{load_policy, seed_retention_policies, RecordClass};
    use crate::state::AppState;

    const NOW: &str = "2026-08-02T09:00:00Z";
    const KASNIJE: &str = "2026-09-15T11:30:00Z";

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            seed_retention_policies(&state, NOW).expect("retention classes should seed");
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn sign_in_admin(state: &AppState) -> i64 {
        let admin_id: i64 = state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist");
        state
            .set_session_user_id(admin_id)
            .expect("admin session should set");
        admin_id
    }

    fn sign_in_cashier(state: &AppState) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, pin_hash, active,
                                    created_at, updated_at)
                 VALUES ('radnica', 'Milica Milićević', 'cashier', 'pin-hes',
                         1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
        cashier_id
    }

    /// Req. 22's second limb: the setting exists, and it is the rukovalac's.
    /// Both verbs are gated inside the domain function rather than in the
    /// `#[tauri::command]` wrapper, so no in-process caller routes around it.
    #[test]
    fn only_an_administrator_may_read_or_move_a_rok() {
        with_state("retention_cmd_admin_only", |state| {
            sign_in_cashier(state);

            let read = list_policies(state).expect_err("a kasir may not read the rokovi");
            assert_eq!(read.code(), "forbidden");

            let write = extend_policy(state, "access_log", "2030-01-01", KASNIJE)
                .expect_err("a kasir may not move a rok");
            assert_eq!(write.code(), "forbidden");

            let connection = state.db().open().expect("database should open");
            let untouched = load_policy(&connection, RecordClass::AccessLog)
                .expect("the class must still load");
            assert_eq!(
                untouched.retain_until.as_deref(),
                Some("2028-08-02"),
                "a refused call must leave the stored floor alone"
            );
        });
    }

    /// Req. 6 + req. 22: a configurable period with a documented default, and the
    /// only direction it may ever move. „Rok se pomera samo unapred“ is printed
    /// on the čl. 23 notice handed to the employee, so a shortening must be
    /// refused rather than clamped.
    #[test]
    fn the_shop_moves_a_bounded_rok_forward_and_never_back() {
        with_state("retention_cmd_moves_forward", |state| {
            sign_in_admin(state);

            let seeded = list_policies(state).expect("the rokovi should list");
            let access_log = seeded
                .iter()
                .find(|policy| policy.record_class == RecordClass::AccessLog)
                .expect("the access log class must be listed");
            assert_eq!(
                access_log.retain_until.as_deref(),
                Some("2028-08-02"),
                "the documented default is two years from the seeding day"
            );
            assert!(access_log.adjustable, "req. 22 — this class is the setting");
            assert!(!access_log.never_purge);

            let moved = extend_policy(state, "access_log", "2029-01-01", KASNIJE)
                .expect("the shop should be able to keep the log longer");
            assert_eq!(moved.retain_until.as_deref(), Some("2029-01-01"));
            assert_eq!(moved.record_class, RecordClass::AccessLog);
            assert_eq!(moved.updated_at, KASNIJE, "the row moved, and says when");

            let back = extend_policy(state, "access_log", "2028-12-31", KASNIJE)
                .expect_err("a shortening must be refused, never clamped");
            assert_eq!(back.code(), "validation_error");

            let connection = state.db().open().expect("database should open");
            let stored = load_policy(&connection, RecordClass::AccessLog)
                .expect("the class must still load");
            assert_eq!(
                stored.retain_until.as_deref(),
                Some("2029-01-01"),
                "a refused shortening must leave the stored floor alone"
            );

            let malformed = extend_policy(state, "access_log", "01.01.2030.", KASNIJE)
                .expect_err("a rok that is not gggg-MM-dd must be refused");
            assert_eq!(malformed.code(), "validation_error");
        });
    }

    /// Req. 26 and ZZPL čl. 47 st. 7: the `trajno` classes have no rok to move,
    /// and „every concrete date is shorter than that“. The refusal is a type
    /// boundary — [`AdjustableClass`] has no variant naming them — and the
    /// command has to say so in words rather than silently doing nothing.
    #[test]
    fn a_trajno_class_has_no_rok_that_could_be_moved() {
        with_state("retention_cmd_refuses_trajno", |state| {
            sign_in_admin(state);

            for class in RecordClass::ALL
                .into_iter()
                .filter(|class| class.never_purge())
            {
                let Err(error) = extend_policy(state, class.key(), "2099-12-31", KASNIJE) else {
                    panic!("„{}“ se čuva trajno i mora da odbije rok", class.key());
                };
                assert_eq!(error.code(), "validation_error");
                assert!(
                    error.to_string().contains("trajno"),
                    "the refusal must say why: {error}"
                );

                let connection = state.db().open().expect("database should open");
                let stored = load_policy(&connection, class).expect("the class must still load");
                assert_eq!(
                    stored.retain_until, None,
                    "trajno is the absence of an end date and stays that way"
                );
            }

            let unknown = extend_policy(state, "nesto_izmisljeno", "2099-12-31", KASNIJE)
                .expect_err("an unknown class must be refused");
            assert_eq!(unknown.code(), "validation_error");
        });
    }

    /// Req. 22's third limb. Čl. 47 st. 1 t. 6 asks for the period *per vrsta
    /// podataka*, and the register is the document the Poverenik reads: a rok the
    /// shop chose and the register never disclosed is the gap this limb closes.
    #[test]
    fn the_moved_rok_reaches_the_cl_47_register() {
        with_state("retention_cmd_reaches_the_register", |state| {
            sign_in_admin(state);
            cl47::generate(state, NOW).expect("the register should generate");

            extend_policy(state, "access_log", "2031-03-01", KASNIJE).expect("the rok should move");

            let register = cl47::list(state).expect("the register should list");
            let pristup = register
                .iter()
                .find(|activity| activity.kljuc == "evidencija_pristupa")
                .expect("the register must carry the evidencija pristupa");
            let rok = pristup.rok_cuvanja.as_deref().expect("a rok");
            assert!(
                rok.contains("2031-03-01"),
                "the register must print the rok the shop chose, not the seeded one: {rok}"
            );
        });
    }

    /// The change is a radnja over personal data in its own right — it decides
    /// how long the evidencija pristupa itself survives — so it goes into the
    /// evidencija pristupa. Req. 4: the row id and nothing else.
    #[test]
    fn moving_a_rok_is_written_into_the_evidencija_pristupa() {
        with_state("retention_cmd_logs_the_change", |state| {
            let admin_id = sign_in_admin(state);

            extend_policy(state, "access_log", "2029-01-01", KASNIJE).expect("the rok should move");

            let connection = state.db().open().expect("database should open");
            let mut statement = connection
                .prepare(
                    "SELECT action, object_type, object_id, actor_user_id
                     FROM audit_events ORDER BY id",
                )
                .expect("statement should prepare");
            let rows: Vec<(String, String, String, Option<i64>)> = statement
                .query_map([], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })
                .expect("query should run")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("rows should read");

            let logged = rows
                .iter()
                .find(|(_, object_type, _, _)| object_type == "retention_policy")
                .expect("moving a rok must be recorded in the evidencija pristupa");
            assert_eq!(logged.0, "menjanje");
            assert_eq!(logged.3, Some(admin_id));
            assert!(
                logged.2.chars().all(|character| character.is_ascii_digit()),
                "req. 4 — the line carries the row id and nothing else: {}",
                logged.2
            );
        });
    }

    /// The whole point of an exhaustive list. A class that is not `trajno` and
    /// that no command can move is a class whose „podesiv“ prose is a promise
    /// nothing keeps — which is the defect this module was built to end.
    #[test]
    fn every_class_that_is_not_trajno_is_named_by_an_adjustable_variant() {
        let movable: Vec<RecordClass> = AdjustableClass::ALL
            .into_iter()
            .map(AdjustableClass::record_class)
            .collect();
        let bounded: Vec<RecordClass> = RecordClass::ALL
            .into_iter()
            .filter(|class| !class.never_purge())
            .collect();

        assert_eq!(
            movable, bounded,
            "every bounded class needs a command that can move its rok, and no trajno class \
             may appear here"
        );

        for class in AdjustableClass::ALL {
            assert_eq!(
                AdjustableClass::from_key(class.record_class().key()),
                Some(class),
                "{} must round-trip through its stored key",
                class.record_class().key()
            );
        }
        assert_eq!(AdjustableClass::from_key("personnel"), None);
        assert_eq!(AdjustableClass::from_key("nesto_izmisljeno"), None);
    }

    /// The reading half of the setting: the operator has to see the rok that is
    /// in force and the class it belongs to before deciding to move it.
    #[test]
    fn the_list_shows_every_class_with_its_rok_and_whether_it_can_move() {
        with_state("retention_cmd_lists_every_class", |state| {
            sign_in_admin(state);

            let listed = list_policies(state).expect("the rokovi should list");
            assert_eq!(
                listed.len(),
                RecordClass::ALL.len(),
                "the operator sees the whole table, not only the adjustable rows"
            );

            for policy in &listed {
                assert_eq!(
                    policy.adjustable,
                    AdjustableClass::from_key(policy.record_class.key()).is_some(),
                    "„podesiv“ on screen must mean a command can actually move it: {}",
                    policy.record_class.key()
                );
                assert!(
                    !policy.naziv.is_empty(),
                    "every class needs a name a person can read: {}",
                    policy.record_class.key()
                );
                assert_eq!(policy.napomena, policy.record_class.napomena());

                if policy.never_purge {
                    assert!(
                        !policy.adjustable,
                        "a trajno class is never adjustable: {}",
                        policy.record_class.key()
                    );
                    assert_eq!(policy.retain_until, None);
                }
            }
        });
    }
}
