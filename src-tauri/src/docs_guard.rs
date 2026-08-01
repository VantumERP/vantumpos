//! Guards over the compliance prose that describes this crate.
//!
//! `docs/SERBIAN-LAW-COMPLIANCE.md`, `docs/PROGRESS.md` and the templates under
//! `docs/compliance/` are read as a description of what the code does — by the
//! founder, by whoever advises the shop, and eventually by an inspector. A row
//! that credits a module with a statutory leg the module does not implement is
//! a compliance defect in its own right: it withdraws the only pointer to a real
//! gap, and it withdraws it first for the provisions protecting the most exposed
//! people. Four such claims shipped with SW-14 and nothing could see them,
//! because nothing in this crate had ever read the documents.
//!
//! Each guard below pins one prose claim against the code that has to back it.
//! **When a leg is genuinely built, delete its guard and re-state the prose in
//! the same commit** — never relax an assertion to make a stale sentence pass.
//!
//! Test-only: the module is declared `#[cfg(test)]` in `lib.rs`, so the
//! documents are embedded in the test binary and never in the shipped app.

const REGISTER: &str = include_str!("../../docs/SERBIAN-LAW-COMPLIANCE.md");
const PROGRESS: &str = include_str!("../../docs/PROGRESS.md");
const NOTICE: &str = include_str!("../../docs/compliance/obavestenje-zaposlenima.md");

/// Every line of `text` containing `needle`, numbered from 1 the way an editor
/// numbers them so a failure message points straight at the line to fix.
fn lines_with<'a>(text: &'a str, needle: &str) -> Vec<(usize, &'a str)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(index, line)| (index + 1, line))
        .collect()
}

/// The register and `PROGRESS.md` both credited `worktime::check_protection`
/// with „maloletnik 35 h/8 h“. Only the daily leg exists — see
/// `worktime::MINOR_DAILY_CAP_MINUTES` and the doc comment on
/// `check_protection`, which says the weekly leg needs a week the signature does
/// not carry. Naming the figure is fine; asserting it is enforced is not, so any
/// line carrying it must mark it unbuilt on the same line.
#[test]
fn no_document_claims_the_cl_87_weekly_leg_is_enforced() {
    const UNBUILT: [&str; 3] = ["is not checked", "nije proveren", "not implemented"];

    for (doc, text) in [
        ("docs/SERBIAN-LAW-COMPLIANCE.md", REGISTER),
        ("docs/PROGRESS.md", PROGRESS),
    ] {
        for needle in ["35 h", "35 časova"] {
            for (line_no, line) in lines_with(text, needle) {
                assert!(
                    UNBUILT.iter().any(|marker| line.contains(marker)),
                    "{doc}:{line_no} names the ZoR čl. 87 weekly leg (\"{needle}\") without \
                     saying it is unbuilt. Only the 8 h/day leg is in code \
                     (worktime::MINOR_DAILY_CAP_MINUTES); the 35 h/week leg is enforced nowhere."
                );
            }
        }
    }
}

/// `worktime::assess_caps` hard-codes `preraspodela_weekly_cap_exceeded` to
/// `false` — it cannot see whether the employee is in preraspodela. The čl. 57
/// st. 5 branch is taken by the caller,
/// `commands::worktime::assess_caps_for_employee`. Pointing an auditor at
/// `assess_caps` for that leg sends them to a function that provably returns
/// `false` for it, so wherever the register puts the two side by side it must
/// also name the caller.
#[test]
fn the_cl_57_st_5_ceiling_is_attributed_to_its_caller() {
    for (line_no, line) in lines_with(REGISTER, "57 st. 5") {
        if !line.contains("assess_caps") {
            continue;
        }
        assert!(
            line.contains("commands/worktime.rs"),
            "docs/SERBIAN-LAW-COMPLIANCE.md:{line_no} attributes ZoR čl. 57 st. 5 to \
             `assess_caps`, which always leaves that leg false. The branch is taken in \
             commands/worktime.rs::assess_caps_for_employee — name it on the same line."
        );
    }
}

/// The SW-14 row asserted that „no fine figure exists outside `legal.rs`“. That
/// is the target invariant, not the state of the tree: `ReklamacijeModule.tsx`
/// still hard-codes the regime-versioned reklamacija amounts, which the SW-15
/// row seven lines below documents as an open SW-7 follow-up. A global claim
/// deletes the only pointer to it, so the claim must be scoped to what SW-14
/// shipped or carry the exception with it.
#[test]
fn the_no_fine_figure_claim_stays_scoped_to_what_sw_14_shipped() {
    for (line_no, line) in lines_with(REGISTER, "fine figure") {
        if !line.contains("outside `legal.rs`") {
            continue;
        }
        assert!(
            line.contains("SW-14 introduces") || line.contains("ReklamacijeModule"),
            "docs/SERBIAN-LAW-COMPLIANCE.md:{line_no} claims globally that no fine figure lives \
             outside legal.rs. src/app/reklamacije/ReklamacijeModule.tsx hard-codes the \
             regime-versioned reklamacija amounts — scope the claim to SW-14 or carry the \
             known-exception pointer."
        );
    }
}

/// Failing to deliver the ZZPL čl. 23 notice is čl. 95 st. 1 **tač. 8** („licu
/// na koje se podaci odnose ne pruži informacije iz člana 23. st. 1. do 3.“) —
/// see `docs/SW14-VERIFIED-RULES.md` §3 W5. **tač. 20** is the čl. 42
/// privacy-by-design offence and belongs to register row 12, not to the document
/// handed to an employee.
#[test]
fn the_cl_23_notice_cites_the_right_offence_tacka() {
    assert!(
        NOTICE.contains("čl. 95 st. 1 tač. 8"),
        "docs/compliance/obavestenje-zaposlenima.md must cite ZZPL čl. 95 st. 1 tač. 8 as the \
         offence for failing to deliver the čl. 23 notice"
    );

    let wrong = lines_with(NOTICE, "tač. 20");
    assert!(
        wrong.is_empty(),
        "docs/compliance/obavestenje-zaposlenima.md cites ZZPL čl. 95 st. 1 tač. 20, which is the \
         čl. 42 privacy-by-design offence, not the čl. 23 notice: {wrong:?}"
    );
}
