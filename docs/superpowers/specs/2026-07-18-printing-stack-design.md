# Printing Stack (SW-8) — Design

**Date:** 2026-07-18
**Status:** Approved (open-in-default-browser; both export and print buttons kept)
**Legal authority:** `docs/ZOT-36-37-VERIFIED-RULES.md` §4.6 (Pravilnik čl. 10 st. 3 „štampanje podataka na zahtev"; čl. 18 print-on-demand) and čl. 37 st. 2 (label display duty).
**Builds on:** SW-6c (merged): `campaign_evidence.rs` renders self-contained, print-ready HTML files to `exports/`; export commands return `ExportedFile { path, … }`.

## Scope

A reusable **open-to-print primitive** — the "printing stack" the compliance roadmap names as the shared prerequisite for KEP print-on-demand (SW-9) and potvrda o prijemu (reklamacije SW-7). It opens an already-exported HTML document in the OS default handler (the browser), where the shop prints via Ctrl+P. First consumer: the campaign label and evidence documents.

**Why this shape:** SW-6c already generates the print-ready HTML. The only thing missing is getting it to a printer. Opening the file via `tauri-plugin-opener` reuses all of 6c and adds almost no unverifiable surface — there is no `window.print()` / `@media print` React view whose fidelity cannot be checked without booting the app. The one inherently unverifiable sliver — the OS actually launching the browser and printing — is a single plugin call, and it degrades gracefully.

**Deferred / non-goals:** dedicated in-app print-preview routes, PDF generation, printer-driver selection, label-stock dimension presets (browser print settings handle margins). No new Rust logic. No change to any document's content (that is 6c).

## Locked decisions

| Decision | Choice |
|---|---|
| Print path | Open the exported `.html` in the OS default browser via `@tauri-apps/plugin-opener` (`openPath`). Shop prints with Ctrl+P. |
| Buttons | **Keep** the existing export-only buttons; **add** parallel „Štampaj …" buttons (export → open). |
| Primitive shape | A reusable `PrintService.openForPrint(path)`, so SW-7/SW-9 reuse it. |
| Failure handling | If opening fails, toast the saved file path so the shop opens it manually — the export already wrote the file, so nothing is lost. |

## 1. Capability

`src-tauri/capabilities/default.json` — add `"opener:allow-open-path"` to `permissions`. `opener:default` already grants `open-url` / `reveal-item-in-dir` / `default-urls`, but **not** opening a file path; `openPath` requires `allow-open-path` (verified against tauri-plugin-opener 2.5.4: its `default` set omits it, and `allow-open-path` "enables the open_path command without any pre-configured scope"). `cargo build` (via `tauri-build`) validates the permission id, so a typo fails the build — this line is verifiable.

Scope note: `allow-open-path` is unscoped. The app only ever calls it with app-generated paths under `exports/`, so unscoped is acceptable for this cycle; a path-scoped variant is a possible later hardening, not required now.

## 2. `PrintService` (frontend)

New service on `PosServices`:

```ts
export interface PrintService {
  /** Opens an exported document in the OS default handler for printing. */
  openForPrint(path: string): Promise<void>;
}
```

- **Local adapter** (`local-adapter.ts`): `openForPrint: (path) => openPath(path)` where `openPath` is imported from `@tauri-apps/plugin-opener` (package already in `package.json`). This is a direct plugin call, not a Tauri `invoke` — the adapter boundary already isolates such specifics.
- **Mock adapter** (`mock-adapter.ts`): `openForPrint: async () => {}` (no-op), so component tests can spy on it.
- `PosServices` gains `print: PrintService`.

## 3. Campaign consumers

`src/app/campaigns/CampaignsModule.tsx` gains a `runPrint` sibling to the existing `runExport`:

```ts
async function runPrint(action, fallback) {
  let exported;
  try {
    exported = await action();                 // reuse the existing 6c export command
  } catch (error) {
    toast.error("Izvoz nije uspeo", { description: errorMessage(error, fallback) });
    return;
  }
  try {
    await printService.openForPrint(exported.path);
    toast.success("Otvoreno za štampu", { description: exported.path });
  } catch {
    // The file is saved; only the open failed. Point the shop at it.
    toast.warning("Dokument je sačuvan — otvorite ga ručno za štampu", {
      description: exported.path,
    });
  }
}
```

Buttons (Serbian, verbatim), each beside its existing „Izvezi …" counterpart:
- Campaign detail: **„Štampaj dokaz o ceni"** → `runPrint(() => exportEvidence(id), …)`; **„Štampaj etikete"** → `runPrint(() => exportLabels(id), …)`. Threaded as `onPrintEvidence` / `onPrintLabels` props on the detail component (mirroring `onExportEvidence` / `onExportLabels`).
- Correction panel: **„Štampaj"** → `runPrint(() => exportCorrectionReport(), …)`, as an `onPrint` prop beside `onExport`.

`print` is passed into `CampaignsModule` via the existing `services` prop.

## 4. Testing

- **Adapter mapping:** `openForPrint(path)` calls the opener plugin's `openPath` with that path (mock the module; `local-adapter.test.ts` pattern).
- **RTL — happy path:** clicking „Štampaj etikete" calls `exportLabels(id)`, then `print.openForPrint` with the returned `path`, and toasts success.
- **RTL — export fails:** the error toast fires and `openForPrint` is **not** called.
- **RTL — open fails:** `exportLabels` resolves but `openForPrint` rejects → the warning toast carries the saved path (the file-is-saved fallback).
- **Mock services** expose `print.openForPrint` so existing suites keep type-checking (add `print` to every `createMockServices`/`PosServices` construction).
- **Capability:** `cargo build` succeeds with the new permission (invalid id would fail the build).

**Explicitly unverifiable here (inherent, one line):** that the OS actually opens the browser and renders the print dialog. Everything else is covered.

## Acceptance criteria
- Any campaign label / evidence / correction document can be opened for printing in one click, reusing the SW-6c export unchanged.
- Both the export-only and print buttons are present.
- An open failure never loses the document — the saved path is surfaced.
- `openForPrint` is a reusable primitive on `PosServices` (SW-7/SW-9 will reuse it).
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`, and `cargo build` (capability validation).
