# Module Spec: Settings, VAT, Receipt Numbering, Users, And Backup

## Goal

Implement Podesavanja for local shop configuration and backup/restore. Because the app is local-first, backup is not optional.

## Scope

In scope:

- Company/shop profile.
- VAT rates.
- Receipt numbering.
- Default currency display.
- Users and roles entry point if not implemented in Auth module.
- Backup folder setting.
- Manual backup.
- Automatic backup status.
- Restore from backup with confirmation.

Out of scope:

- Cloud backup.
- Fiscal device configuration.
- Multi-store settings.
- Full accounting settings.

## Screens

Use `Tabs`:

- Radnja,
- PDV,
- Racuni,
- Korisnici,
- Backup.

### Shop Profile

Fields:

- shop/company name,
- address,
- PIB,
- registration number,
- phone,
- logo path optional for later.

### VAT Rates

Use `Table` and `Dialog`/`Sheet`.

Fields:

- name,
- rate basis points,
- active.

Do not allow deleting VAT rates used by products/sales. Deactivate instead.

### Receipt Numbering

Fields:

- prefix,
- next sequence number,
- reset policy for MVP should be explicit: no automatic reset unless implemented and tested.

### Backup

Fields/status:

- backup folder,
- last successful backup,
- last failed backup,
- automatic backup enabled,
- manual backup button,
- restore button.

Restore must use `AlertDialog` and require clear confirmation.

## Service Contract

```ts
interface SettingsService {
  getHealth(): Promise<AppHealth>
  getCompanySettings(): Promise<CompanySettings>
  updateCompanySettings(request: CompanySettingsRequest): Promise<CompanySettings>
  listTaxRates(): Promise<TaxRate[]>
  saveTaxRate(request: SaveTaxRateRequest): Promise<TaxRate>
  getReceiptSettings(): Promise<ReceiptSettings>
  updateReceiptSettings(request: ReceiptSettingsRequest): Promise<ReceiptSettings>
}

interface BackupService {
  getBackupStatus(): Promise<BackupStatus>
  createBackup(request: CreateBackupRequest): Promise<BackupJob>
  restoreBackup(request: RestoreBackupRequest): Promise<BackupJob>
  listBackupJobs(): Promise<BackupJob[]>
}
```

## Backend Commands

- `settings_get_company`
- `settings_update_company`
- `settings_list_tax_rates`
- `settings_save_tax_rate`
- `settings_get_receipt`
- `settings_update_receipt`
- `backup_get_status`
- `backup_create`
- `backup_restore`
- `backup_list_jobs`

## SQLite Notes

Existing tables:

- `settings`
- `tax_rates`
- `backup_jobs`

Use typed JSON values in `settings` for config groups. Keep keys stable:

- `company`
- `receipt_numbering`
- `backup`

Consider migration if `backup_jobs` needs `completed_at`, file size, or checksum.

Use SQLite backup API or a controlled backend backup operation. Do not copy the database file while writes may be active.

## Business Rules

- PIB format validation can be basic in MVP, but field should exist.
- Currency is `RSD` and should not be freely changed in MVP.
- VAT rates used by products cannot be hard-deleted.
- Receipt sequence updates require admin role.
- Restore replaces current local data and must require confirmation.
- Restore should create a pre-restore backup first if feasible.

## Tests

Backend:

- settings get/update round trip,
- VAT rate create/update,
- used VAT rate cannot be deleted if delete exists,
- receipt sequence update persists,
- backup job records success,
- restore rejects missing/unreadable file.

Frontend:

- settings tabs render distinct content,
- company form saves and shows success toast,
- VAT validation errors render,
- backup status renders stale/failed states,
- restore confirmation is required.

## Acceptance Criteria

- Podesavanja is a real admin screen.
- Shop profile and VAT settings persist.
- Manual backup works.
- Restore is guarded by confirmation.
- Backup status is available for shell warning.

## Prompt For Separate Chat

```text
Implementiraj Settings/Podesavanja and Backup module for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/08-settings-backup.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build settings tabs, company settings, VAT rates, receipt numbering, backup status, manual backup, guarded restore, service contracts, Tauri commands, SQLite persistence, Rust tests, frontend tests, and mock adapter data. Do not add cloud backup or fiscal device configuration.
```

