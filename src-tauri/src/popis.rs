//! The popis state machine and the PoP čl. 8 st. 5 blind-count property (SW-16).
//!
//! Legal authority: `docs/REMAINING-SW-VERIFIED-RULES.md` §2c and §4 reqs. 29–42.
//! Design: `docs/superpowers/specs/2026-08-01-sw16-popis-design.md`.
//!
//! The module is this state machine. Consumed by the command layer, so
//! `dead_code` is allowed here — mirroring the other domain modules.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::app_error::AppError;

/// The six states of a popis, in the bylaw's own sequence. The stored values are
/// [`PopisStatus::as_db_str`] and they are the vocabulary of the v20
/// `popis_sessions.status` CHECK — a state this enum did not model would be
/// refused by neither the blind-count guard nor the posting lock, so the two
/// lists are asserted equal by test rather than kept in step by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopisStatus {
    /// The odluka, the komisija and the plan rada (čl. 8 st. 1–2) exist; nothing
    /// has been counted.
    Draft,
    /// Phase A. The natural count of čl. 9 st. 1 t. 1 is under way.
    Counting,
    /// The čl. 8 st. 5 signature is on the counted state. **This is the moment
    /// the book quantities may be released**, and nothing earlier is.
    CountedSigned,
    /// Phase B. Differences and valuation, čl. 9 st. 1 t. 3–6.
    Computed,
    /// The čl. 9 st. 3 signature on the printed, computed liste.
    ComputedSigned,
    /// Čl. 14 st. 3 — the result is knjižen. The last state; a correction is a
    /// new popis (ZoRač čl. 8 st. 4).
    Posted,
}

impl PopisStatus {
    pub const ALL: [Self; 6] = [
        Self::Draft,
        Self::Counting,
        Self::CountedSigned,
        Self::Computed,
        Self::ComputedSigned,
        Self::Posted,
    ];

    /// The value stored in `popis_sessions.status`.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Counting => "counting",
            Self::CountedSigned => "counted_signed",
            Self::Computed => "computed",
            Self::ComputedSigned => "computed_signed",
            Self::Posted => "posted",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_db_str() == value)
    }

    /// What the state is called in front of the shop owner. Deliberately not the
    /// stored key: „counted_signed“ names a column value, and a refusal that
    /// quotes one is a refusal nobody in the shop can act on.
    pub fn naziv(self) -> &'static str {
        match self {
            Self::Draft => "priprema popisa",
            Self::Counting => "brojanje",
            Self::CountedSigned => "stvarno stanje potpisano",
            Self::Computed => "obračun razlika",
            Self::ComputedSigned => "obračunate liste potpisane",
            Self::Posted => "proknjižen popis",
        }
    }
}

/// The five events that move a popis. One per statutory step that changes the
/// legal character of the document — counting, the two potpisi, the obračun and
/// the knjiženje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopisEvent {
    /// Open Phase A: the commission starts the natural count (čl. 9 st. 1 t. 1).
    Count,
    /// The čl. 8 st. 5 signature on the counted state.
    SignA,
    /// Enter Phase B: differences and valuation (čl. 9 st. 1 t. 3–6).
    Compute,
    /// The čl. 9 st. 3 signature on the printed, computed liste.
    SignB,
    /// Čl. 14 st. 3 — book the result.
    Post,
}

impl PopisEvent {
    pub const ALL: [Self; 5] = [
        Self::Count,
        Self::SignA,
        Self::Compute,
        Self::SignB,
        Self::Post,
    ];

    pub fn naziv(self) -> &'static str {
        match self {
            Self::Count => "početak brojanja",
            Self::SignA => "potpis stvarnog stanja (čl. 8 st. 5)",
            Self::Compute => "obračun razlika",
            Self::SignB => "potpis obračunatih listi (čl. 9 st. 3)",
            Self::Post => "knjiženje rezultata (čl. 14 st. 3)",
        }
    }
}

/// The only legal path through a popis: `draft → counting → counted_signed →
/// computed → computed_signed → posted`, one event per arrow. Every other pair
/// is refused, and two refusals carry their own code because they are the two a
/// shop will actually meet.
///
/// The refusal that matters is `Compute` out of Phase A. Čl. 9 st. 1 orders the
/// steps and čl. 8 st. 5 makes the order a duty: the obračun reads book
/// quantities, so it may not begin until the counted state is written into the
/// liste and signed. Refusing it here is what keeps Phase B unreachable by
/// ordering rather than by screen sequence.
///
/// This function decides on `current` alone, which is a claim the caller makes.
/// It is therefore the *ordering* half of čl. 8 st. 5 and not the whole of it —
/// see [`book_quantities_released`] for the release condition, and the v20
/// triggers for the write refusal that stands behind both.
pub fn advance(current: PopisStatus, event: PopisEvent) -> Result<PopisStatus, AppError> {
    // Checked before the sequence, so a posted popis answers „the popis is
    // closed“ to every event rather than „that step is out of order“ — the same
    // order, and the same wording, the v20 posting lock uses.
    if current == PopisStatus::Posted {
        return Err(AppError::business(
            "popis_proknjizen",
            "Proknjižen popis se ne menja — ispravka se sprovodi novim popisom.",
        ));
    }

    match (current, event) {
        (PopisStatus::Draft, PopisEvent::Count) => Ok(PopisStatus::Counting),
        (PopisStatus::Counting, PopisEvent::SignA) => Ok(PopisStatus::CountedSigned),
        (PopisStatus::CountedSigned, PopisEvent::Compute) => Ok(PopisStatus::Computed),
        (PopisStatus::Computed, PopisEvent::SignB) => Ok(PopisStatus::ComputedSigned),
        (PopisStatus::ComputedSigned, PopisEvent::Post) => Ok(PopisStatus::Posted),
        (PopisStatus::Draft | PopisStatus::Counting, PopisEvent::Compute) => {
            Err(AppError::business(
                "popis_not_signed",
                "Obračun ne može da počne pre nego što se stvarno stanje unese u popisne liste \
                 i pre nego što članovi komisije potpišu te liste (PoP čl. 8 st. 5).",
            ))
        }
        _ => Err(AppError::business(
            "popis_nedozvoljen_prelaz",
            format!(
                "Korak „{}“ nije moguć iz stanja „{}“.",
                event.naziv(),
                current.naziv()
            ),
        )),
    }
}

/// The **status limb** of PoP čl. 8 st. 5 — false for every state in Phase A,
/// true from the counted state onward. Exactly the vocabulary the v20
/// `popis_lines` triggers refuse a book quantity in (`draft`, `counting`), so
/// the pure predicate and the engine guard cannot disagree about which states
/// are blind.
///
/// **This is one of the two limbs and must not be used alone to release book
/// data.** `status` is a claim any UPDATE can make, and a session can be born in
/// `counted_signed` with no potpis behind it — which is precisely the failure
/// čl. 8 st. 5 names. Call [`book_quantities_released`] instead wherever the
/// answer decides whether the commission gets the data; this predicate is for
/// the case where the potpis is already known to exist, and for reasoning about
/// the states themselves.
pub fn book_quantities_visible(status: PopisStatus) -> bool {
    !matches!(status, PopisStatus::Draft | PopisStatus::Counting)
}

/// The whole čl. 8 st. 5 release condition. The article makes both facts
/// conditions — „пре уписивања стварног стања … и пре него што чланови комисије
/// … потпишу те листе“ — so both are required: the session must have left Phase
/// A **and** a `faza = 'a'` signature must exist for it. The čl. 9 st. 3 potpis
/// is a different event and does not stand in for it.
pub fn book_quantities_released(status: PopisStatus, phase_a_signed: bool) -> bool {
    book_quantities_visible(status) && phase_a_signed
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::{
        advance, book_quantities_released, book_quantities_visible, PopisEvent, PopisStatus,
    };
    use crate::app_error::AppError;
    use crate::db::{test_database_path, Db};

    fn with_test_database(test_name: &str, test: impl FnOnce(&Connection)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let connection = db.open().expect("database should open");
            test(&connection);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// Seed one popis session directly in `status`. `posted_at` follows the
    /// status because the v20 schema binds the two.
    fn seed_session(connection: &Connection, id: i64, status: PopisStatus) {
        let posted_at: Option<&str> =
            (status == PopisStatus::Posted).then_some("2027-01-15T18:00:00Z");

        connection
            .execute(
                "INSERT INTO popis_sessions (id, vrsta, prodajno_mesto, datum_popisa, status,
                                             posted_at, created_at, updated_at)
                 VALUES (?1, 'godisnji', 'Butik Centar', '2026-12-31', ?2, ?3,
                         '2026-12-31T08:00:00Z', '2026-12-31T08:00:00Z')",
                params![id, status.as_db_str(), posted_at],
            )
            .unwrap_or_else(|error| panic!("a session in {status:?} should insert: {error}"));
    }

    /// Take the čl. 8 st. 5 potpis — the second limb of the release condition.
    fn sign_phase_a(connection: &Connection, session_id: i64) {
        connection
            .execute(
                "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                               snapshot_hash, created_at)
                 VALUES (?1, 'a', 'Miloš Đurđević', '2026-12-31T17:00:00Z', 'hash-a',
                         '2026-12-31T17:00:00Z')",
                params![session_id],
            )
            .unwrap_or_else(|error| panic!("the čl. 8 st. 5 signature should record: {error}"));
    }

    /// Try to store a book quantity — the payload čl. 8 st. 5 governs.
    fn write_book_quantity(connection: &Connection, session_id: i64) -> rusqlite::Result<usize> {
        connection.execute(
            "INSERT INTO popis_lines (session_id, lista_vrsta, naziv, stvarna_kolicina_milli,
                                      knjigovodstvena_kolicina_milli, created_at, updated_at)
             VALUES (?1, 'roba', 'Košulja', 7000, 6000,
                     '2026-12-31T09:00:00Z', '2026-12-31T09:00:00Z')",
            params![session_id],
        )
    }

    fn refusal_message(result: rusqlite::Result<usize>) -> String {
        match result {
            Err(rusqlite::Error::SqliteFailure(_, Some(message))) => message,
            other => panic!("expected the write to be refused, got {other:?}"),
        }
    }

    /// The `'…'` tokens of the `popis_sessions.status` CHECK, read back off the
    /// live schema rather than copied out of the migration source.
    fn status_check_vocabulary(connection: &Connection) -> Vec<String> {
        let schema: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'popis_sessions'",
                [],
                |row| row.get(0),
            )
            .expect("popis_sessions schema should load");

        let opening = "CHECK (status IN (";
        let start = schema
            .find(opening)
            .expect("popis_sessions must carry a closed status CHECK")
            + opening.len();
        let rest = &schema[start..];
        let end = rest.find("))").expect("the status CHECK should close");

        let mut values: Vec<String> = rest[..end]
            .split(',')
            .map(|token| token.trim().trim_matches('\'').to_string())
            .collect();
        values.sort();
        values
    }

    /// PoP čl. 8 st. 5: book quantities must not reach the commission before the
    /// counted state is written and signed. A UI that merely hides them is not
    /// enough — the property belongs to the state machine.
    #[test]
    fn book_quantities_are_invisible_until_phase_a_is_signed() {
        for status in [PopisStatus::Draft, PopisStatus::Counting] {
            assert!(!book_quantities_visible(status), "{status:?} must be blind");
        }
        for status in [
            PopisStatus::CountedSigned,
            PopisStatus::Computed,
            PopisStatus::ComputedSigned,
            PopisStatus::Posted,
        ] {
            assert!(book_quantities_visible(status), "{status:?} may reveal");
        }
    }

    #[test]
    fn phase_b_cannot_be_entered_without_the_phase_a_signature() {
        let error = advance(PopisStatus::Counting, PopisEvent::Compute)
            .expect_err("computing before the čl. 8 st. 5 signature must be refused");
        assert!(matches!(
            error,
            AppError::Business {
                code: "popis_not_signed",
                ..
            }
        ));
    }

    #[test]
    fn a_posted_popis_admits_no_further_transition() {
        for event in [
            PopisEvent::Count,
            PopisEvent::Compute,
            PopisEvent::SignA,
            PopisEvent::SignB,
            PopisEvent::Post,
        ] {
            assert!(
                advance(PopisStatus::Posted, event).is_err(),
                "{event:?} after posting"
            );
        }
    }

    /// Čl. 9 st. 1 orders the steps and čl. 8 st. 5 makes the order a duty, so
    /// the sequence is walked end to end rather than spot-checked.
    #[test]
    fn the_bylaw_sequence_is_the_only_path_to_posted() {
        let mut status = PopisStatus::Draft;

        for (event, expected) in [
            (PopisEvent::Count, PopisStatus::Counting),
            (PopisEvent::SignA, PopisStatus::CountedSigned),
            (PopisEvent::Compute, PopisStatus::Computed),
            (PopisEvent::SignB, PopisStatus::ComputedSigned),
            (PopisEvent::Post, PopisStatus::Posted),
        ] {
            status = advance(status, event)
                .unwrap_or_else(|error| panic!("{event:?} should be legal here: {error}"));
            assert_eq!(status, expected, "after {event:?}");
        }
    }

    /// Exhaustive over all 30 (status, event) pairs: the five arrows above are
    /// the whole machine and every one of the other 25 is refused. A spot-check
    /// would leave the interesting ones — signing B before A, re-counting a
    /// counted popis, posting an unsigned one — to be discovered in the shop.
    #[test]
    fn every_pair_outside_the_sequence_is_refused() {
        let sequence = [
            (PopisStatus::Draft, PopisEvent::Count),
            (PopisStatus::Counting, PopisEvent::SignA),
            (PopisStatus::CountedSigned, PopisEvent::Compute),
            (PopisStatus::Computed, PopisEvent::SignB),
            (PopisStatus::ComputedSigned, PopisEvent::Post),
        ];

        for status in PopisStatus::ALL {
            for event in PopisEvent::ALL {
                if sequence.contains(&(status, event)) {
                    continue;
                }

                let error = advance(status, event)
                    .expect_err("only the bylaw's own sequence may be walked");
                let expected_code = match (status, event) {
                    (PopisStatus::Posted, _) => "popis_proknjizen",
                    (PopisStatus::Draft | PopisStatus::Counting, PopisEvent::Compute) => {
                        "popis_not_signed"
                    }
                    _ => "popis_nedozvoljen_prelaz",
                };
                assert_eq!(error.code(), expected_code, "{status:?} + {event:?}");
            }
        }
    }

    /// Req. 29's actual shape. `status` is a claim any UPDATE can make — a
    /// session can be born in `counted_signed` — so the status limb alone would
    /// hand the book data to a commission that never signed anything, which is
    /// the failure čl. 8 st. 5 names. Both limbs, always.
    #[test]
    fn the_status_limb_alone_does_not_release_the_book_data() {
        for status in PopisStatus::ALL {
            assert!(
                !book_quantities_released(status, false),
                "{status:?} without the čl. 8 st. 5 potpis must stay blind"
            );
            assert_eq!(
                book_quantities_released(status, true),
                book_quantities_visible(status),
                "{status:?} with the potpis must follow the status limb"
            );
        }

        assert!(
            book_quantities_visible(PopisStatus::CountedSigned)
                && !book_quantities_released(PopisStatus::CountedSigned, false),
            "a session claiming counted_signed with no potpis behind it must be refused the data"
        );
    }

    /// The enum and the v20 CHECK are one vocabulary. A state the engine stores
    /// but this enum cannot name would reach [`advance`] as nothing at all, and
    /// a state this enum names but the engine refuses would be a transition the
    /// machine allows and the database rejects.
    #[test]
    fn the_status_vocabulary_is_exactly_the_schema_check() {
        with_test_database("popis_status_vocabulary", |connection| {
            let mut modelled: Vec<String> = PopisStatus::ALL
                .into_iter()
                .map(|status| status.as_db_str().to_string())
                .collect();
            modelled.sort();

            assert_eq!(status_check_vocabulary(connection), modelled);

            for (index, status) in PopisStatus::ALL.into_iter().enumerate() {
                seed_session(connection, 100 + index as i64, status);
                assert_eq!(
                    PopisStatus::from_db_str(status.as_db_str()),
                    Some(status),
                    "{status:?} should survive the round trip through its stored value"
                );
            }

            assert!(
                PopisStatus::from_db_str("counted").is_none(),
                "an unknown stored value must not resolve to a state"
            );
        });
    }

    /// The pure predicate and the v20 write guard must not be able to disagree
    /// about which states are blind: the predicate is what a screen or a query
    /// consults, the guard is what actually stops the row being written, and a
    /// divergence between them is a book quantity in the commission's hands.
    #[test]
    fn the_release_predicate_agrees_with_the_engine_guard() {
        with_test_database("popis_release_predicate", |connection| {
            for (index, status) in PopisStatus::ALL.into_iter().enumerate() {
                if status == PopisStatus::Posted {
                    // Readable (the quantities are in the signed liste) but not
                    // writable: req. 41 closes the row, not the column. The
                    // refusal must say so rather than blame čl. 8 st. 5.
                    seed_session(connection, 300, status);
                    let message = refusal_message(write_book_quantity(connection, 300));
                    assert!(
                        message.contains("Proknjižen popis se ne menja"),
                        "a posted popis must refuse the write as closed, said: {message}"
                    );
                    continue;
                }

                let unsigned = 200 + index as i64 * 2;
                let signed = unsigned + 1;
                seed_session(connection, unsigned, status);
                seed_session(connection, signed, status);
                sign_phase_a(connection, signed);

                assert!(
                    !book_quantities_released(status, false),
                    "{status:?} with no potpis must be blind"
                );
                let message = refusal_message(write_book_quantity(connection, unsigned));
                assert!(
                    message.contains("PoP čl. 8 st. 5"),
                    "{status:?} with no potpis must be refused on čl. 8 st. 5, said: {message}"
                );

                let written = write_book_quantity(connection, signed);
                if book_quantities_released(status, true) {
                    written.unwrap_or_else(|error| {
                        panic!("{status:?} with the potpis should admit the book data: {error}")
                    });
                } else {
                    let message = refusal_message(written);
                    assert!(
                        message.contains("PoP čl. 8 st. 5"),
                        "{status:?} is in Phase A, so even a potpis must not release it, \
                         said: {message}"
                    );
                }
            }
        });
    }
}
