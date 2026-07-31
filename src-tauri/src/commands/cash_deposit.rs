//! Admin-gated command layer over the undeposited-cash aging report (SW-11b).
//!
//! Thin by design: the working-day arithmetic, the FIFO bucket draw-down, the
//! as-of aging and the CSV rendering all live in `crate::cash_deposit`. The
//! admin gate lives there too — inside `cash_deposit_report` itself — so every
//! caller passes through it and these wrappers cannot drift away from it.
//!
//! Neither command blocks anything. Čl. 3 st. 1 is fiscal hygiene supervised by
//! Poreska uprava, not a condition of a valid sale (§3 rule 16).

use std::fs;
use std::path::Path;

use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::cash_deposit::{CashDepositCalendar, CashDepositReport};
use crate::clock::utc_now;
use crate::commands::reports::ExportedFile;
use crate::state::AppState;

#[tauri::command]
pub fn cash_deposit_report(
    state: State<'_, AppState>,
    as_of: String,
) -> Result<CashDepositReport, CommandError> {
    crate::cash_deposit::cash_deposit_report(state.inner(), &as_of).map_err(Into::into)
}

/// Writes the same report as CSV into the `exports/` dir beside the database —
/// the location `reports_export_csv` resolves — and returns the `ExportedFile`
/// descriptor the frontend already understands.
#[tauri::command]
pub fn cash_deposit_export_csv(
    state: State<'_, AppState>,
    as_of: String,
) -> Result<ExportedFile, CommandError> {
    let report = crate::cash_deposit::cash_deposit_report(state.inner(), &as_of)?;
    let csv = crate::cash_deposit::report_to_csv(&report);

    let export_dir = state.db().path().parent().map_or_else(
        || Path::new(".").join("exports"),
        |parent| parent.join("exports"),
    );
    fs::create_dir_all(&export_dir).map_err(AppError::from)?;

    let file_name = format!("nedeponovani-gotov-novac-{as_of}.csv");
    let path = export_dir.join(&file_name);
    fs::write(&path, csv).map_err(AppError::from)?;

    Ok(ExportedFile {
        file_name,
        path: path.display().to_string(),
        mime_type: "text/csv",
        row_count: report.buckets.len(),
    })
}

/// The Settings surface for the calendar the deadline is counted against. The
/// holiday table is `[PRUDENTIAL]` and annual, so it is seeded on first read
/// rather than at install: a shop that upgrades mid-year must not be handed an
/// empty table, and the seed is idempotent, so an admin edit survives it.
#[tauri::command]
pub fn cash_deposit_calendar(
    state: State<'_, AppState>,
) -> Result<CashDepositCalendar, CommandError> {
    let state = state.inner();
    // Gate first: seeding is a write, and an unauthenticated caller must not
    // provoke one.
    crate::commands::auth::require_admin(state)?;
    crate::cash_deposit::seed_default_non_working_days(state, &utc_now()?)?;
    crate::cash_deposit::calendar(state).map_err(Into::into)
}

#[tauri::command]
pub fn cash_deposit_set_saturday_is_working(
    state: State<'_, AppState>,
    counts: bool,
) -> Result<CashDepositCalendar, CommandError> {
    crate::cash_deposit::set_saturday_is_working(state.inner(), counts).map_err(Into::into)
}

#[tauri::command]
pub fn cash_deposit_save_non_working_day(
    state: State<'_, AppState>,
    day: String,
    label: String,
) -> Result<CashDepositCalendar, CommandError> {
    crate::cash_deposit::save_non_working_day(state.inner(), &day, &label, &utc_now()?)
        .map_err(Into::into)
}

#[tauri::command]
pub fn cash_deposit_delete_non_working_day(
    state: State<'_, AppState>,
    day: String,
) -> Result<CashDepositCalendar, CommandError> {
    crate::cash_deposit::delete_non_working_day(state.inner(), &day).map_err(Into::into)
}
