# Printing Stack (SW-8) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the shop open any exported campaign document (labels, price evidence, correction report) in the OS default browser for one-click printing, via a reusable `PrintService` primitive.

**Architecture:** A reusable `PrintService.openForPrint(path)` on `PosServices` calls `@tauri-apps/plugin-opener`'s `openPath` (local adapter) or is a no-op (mock). The campaign detail and correction panel gain „Štampaj …" buttons that reuse the existing SW-6c export commands then open the returned file path; a capability line grants the opener the path permission. No new Rust logic; no print CSS.

**Tech Stack:** React + TypeScript + shadcn/ui + Vitest/RTL; `@tauri-apps/plugin-opener` (already installed); Tauri v2 capability manifest.

## Global Constraints

- **Design authority:** `docs/superpowers/specs/2026-07-18-printing-stack-design.md`.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction. The OS-opens-the-browser behavior is inherently unverifiable here and is out of scope to test.
- **Reuse, don't rebuild:** the export commands (`exportEvidence`/`exportLabels`/`exportCorrectionReport`) and their `ExportedFile { path }` return already exist from SW-6c — print buttons call them unchanged.
- **Keep both buttons:** the existing „Izvezi …" (export-only) buttons stay; „Štampaj …" (export-then-open) buttons are added beside them.
- **Graceful failure:** if opening fails, the document is already saved — toast the saved path so the shop opens it manually.
- Serbian Latin copy with correct diacritics (šđčćž) — exact strings below; copy character-for-character.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`, and `cargo build --manifest-path src-tauri/Cargo.toml` (validates the capability permission id).
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: `PrintService` primitive + capability

**Files:**
- Modify: `src-tauri/capabilities/default.json` (add the permission)
- Modify: `src/services/ports.ts` (new `PrintService`; add to `PosServices`)
- Modify: `src/services/local-adapter.ts` (implement via `openPath`)
- Modify: `src/services/mock-adapter.ts` (no-op)
- Test: `src/services/local-adapter.test.ts`

**Interfaces:**
- Consumes: `@tauri-apps/plugin-opener`'s `openPath(path: string): Promise<void>`.
- Produces (consumed by Task 2):
  ```ts
  export interface PrintService {
    openForPrint(path: string): Promise<void>;
  }
  ```
  and `PosServices.print: PrintService`.

- [ ] **Step 1: Add the capability permission**

In `src-tauri/capabilities/default.json`, add `"opener:allow-open-path"` to the `permissions` array (after `"opener:default"`):

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "opener:allow-open-path"
  ]
}
```

- [ ] **Step 2: Verify the capability builds**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds cleanly. (An invalid permission id would fail `tauri-build`. This is the capability's verification.)

- [ ] **Step 3: Write the failing adapter test**

In `src/services/local-adapter.test.ts`, add a test that mocks the opener module and asserts `openForPrint` forwards the path. Put the `vi.mock` at top level of the file (hoisted):

```ts
vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn().mockResolvedValue(undefined),
}));
```

Then, inside the `describe`:

```ts
  it("opens an exported document for printing through the opener plugin", async () => {
    const { openPath } = await import("@tauri-apps/plugin-opener");
    const services = createLocalServices(vi.fn());
    await services.print.openForPrint("C:/exports/etikete-kampanja-1.html");
    expect(openPath).toHaveBeenCalledWith("C:/exports/etikete-kampanja-1.html");
  });
```

- [ ] **Step 4: Run it to verify it fails**

Run: `bun run test -- local-adapter`
Expected: FAIL — `services.print` is undefined.

- [ ] **Step 5: Define the port**

In `src/services/ports.ts`, add the interface (near the other service interfaces) and extend `PosServices`:

```ts
export interface PrintService {
  /** Opens an exported document in the OS default handler for printing. */
  openForPrint(path: string): Promise<void>;
}
```

Add `print: PrintService;` to the `PosServices` interface.

- [ ] **Step 6: Implement the local adapter**

In `src/services/local-adapter.ts`, add the import at the top:

```ts
import { openPath } from "@tauri-apps/plugin-opener";
```

Add a `print` service to the object returned by `createLocalServices` (alongside `campaigns`):

```ts
    print: {
      openForPrint: (path) => openPath(path),
    },
```

- [ ] **Step 7: Implement the mock adapter**

In `src/services/mock-adapter.ts`, add to the object returned by `createMockServices` (alongside `campaigns`):

```ts
    print: {
      async openForPrint() {},
    },
```

- [ ] **Step 8: Run tests + full gates**

Run: `bun run test -- local-adapter`
Expected: PASS.
Then: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check && git diff --check`
Expected: PASS. (If any other test constructs a bare `PosServices` literal and now fails to type-check for the missing `print`, add `print: { async openForPrint() {} }` there too — `bun run build` / `tsc` will surface them.)

- [ ] **Step 9: Commit**

```bash
git add src-tauri/capabilities/default.json src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/services/local-adapter.test.ts
git commit -m "feat(print): reusable openForPrint primitive + opener path capability (SW-8)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Campaign print buttons

**Files:**
- Modify: `src/app/campaigns/CampaignsModule.tsx`
- Test: `src/app/campaigns/CampaignsModule.test.tsx`

**Interfaces:**
- Consumes: `PrintService.openForPrint` (Task 1); the existing `exportEvidence`/`exportLabels`/`exportCorrectionReport` campaign commands returning `{ path }`; the existing `runExport`/`errorMessage`/`toast` in this file.
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing tests**

In `src/app/campaigns/CampaignsModule.test.tsx`, add (adapt render/service-mock setup to the file's existing pattern — it already builds a mock `services` and selects a campaign to reach the detail view):

```tsx
  it("prints labels by exporting then opening the file", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const exportLabels = vi
      .spyOn(services.campaigns, "exportLabels")
      .mockResolvedValue({
        fileName: "etikete-kampanja-1.html",
        path: "C:/exports/etikete-kampanja-1.html",
        mimeType: "text/html",
        rowCount: 2,
      });
    const openForPrint = vi
      .spyOn(services.print, "openForPrint")
      .mockResolvedValue(undefined);

    // ...render CampaignsModule with these services and open a campaign's detail...

    await user.click(await screen.findByRole("button", { name: "Štampaj etikete" }));

    expect(exportLabels).toHaveBeenCalledWith(expect.any(Number));
    expect(openForPrint).toHaveBeenCalledWith("C:/exports/etikete-kampanja-1.html");
  });

  it("keeps the saved file when opening for print fails", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    vi.spyOn(services.campaigns, "exportLabels").mockResolvedValue({
      fileName: "etikete-kampanja-1.html",
      path: "C:/exports/etikete-kampanja-1.html",
      mimeType: "text/html",
      rowCount: 2,
    });
    vi.spyOn(services.print, "openForPrint").mockRejectedValue(new Error("no handler"));

    // ...render + open detail...

    await user.click(await screen.findByRole("button", { name: "Štampaj etikete" }));

    // The saved path is surfaced so the shop can open it manually.
    expect(await screen.findByText("C:/exports/etikete-kampanja-1.html")).toBeInTheDocument();
  });
```

(If the test file already has a helper that mounts the module and opens a campaign's detail, reuse it. The detail view is reached by clicking a campaign row; follow the existing detail tests in this file for the exact interaction.)

- [ ] **Step 2: Run it to verify it fails**

Run: `bun run test -- CampaignsModule`
Expected: FAIL — no „Štampaj etikete" button.

- [ ] **Step 3: Add the `runPrint` helper**

In `src/app/campaigns/CampaignsModule.tsx`, add `const printService = services.print;` next to the existing `const campaignsService = services.campaigns;` (line 85). Then add this helper beside `runExport` (after it, ~line 266):

```tsx
  async function runPrint(
    action: () => Promise<{ path: string }>,
    fallback: string,
  ) {
    let exported: { path: string };
    try {
      exported = await action();
    } catch (error) {
      toast.error("Izvoz nije uspeo", {
        description: errorMessage(error, fallback),
      });
      return;
    }
    try {
      await printService.openForPrint(exported.path);
      toast.success("Otvoreno za štampu", { description: exported.path });
    } catch {
      toast.warning("Dokument je sačuvan — otvorite ga ručno za štampu", {
        description: exported.path,
      });
    }
  }
```

- [ ] **Step 4: Wire the detail print buttons**

Add `onPrintEvidence` / `onPrintLabels` to the `CampaignDetail` invocation (beside `onExportEvidence` / `onExportLabels`, ~line 394):

```tsx
            onPrintEvidence={() =>
              runPrint(
                () => campaignsService.exportEvidence(selected.id),
                "Dokaz o ceni nije izvezen.",
              )
            }
            onPrintLabels={() =>
              runPrint(
                () => campaignsService.exportLabels(selected.id),
                "Etikete nisu izvezene.",
              )
            }
```

Add the two props to the `CampaignDetail` component's prop type (beside `onExportEvidence: () => void; onExportLabels: () => void;`):

```tsx
  onPrintEvidence: () => void;
  onPrintLabels: () => void;
```

And render the buttons inside the „Dokazi i etikete" group (after the two „Izvezi …" buttons, ~line 785):

```tsx
          <Button type="button" onClick={onPrintEvidence}>
            Štampaj dokaz o ceni
          </Button>
          <Button type="button" onClick={onPrintLabels}>
            Štampaj etikete
          </Button>
```

- [ ] **Step 5: Wire the correction panel print button**

Add `onPrint` to the `CorrectionPanel` invocation (beside `onExport`, ~line 419):

```tsx
        onPrint={() =>
          runPrint(
            () => campaignsService.exportCorrectionReport(),
            "Izveštaj o ispravkama nije izvezen.",
          )
        }
```

Add `onPrint: () => void;` to `CorrectionPanel`'s prop type, and render the button beside „Izvezi":

```tsx
          <Button type="button" onClick={onPrint}>
            Štampaj
          </Button>
```

- [ ] **Step 6: Run the campaign tests**

Run: `bun run test -- CampaignsModule`
Expected: PASS.

- [ ] **Step 7: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check && git diff --check`
Expected: PASS.

```bash
git add src/app/campaigns/CampaignsModule.tsx src/app/campaigns/CampaignsModule.test.tsx
git commit -m "feat(print): Štampaj buttons for campaign labels/evidence/corrections (SW-8)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1 capability → T1 Steps 1–2; §2 `PrintService` → T1 Steps 3–8; §3 consumers (`runPrint` + detail + correction buttons, both-buttons-kept, graceful failure) → T2; §4 testing → the adapter test (T1) + happy/open-fail RTL (T2), capability via `cargo build` (T1 Step 2).
- **Not covered by design (export-fails path):** the spec's §4 lists an "export fails → error toast, openForPrint not called" case. `runPrint` implements it (the first `try/catch` returns before calling `openForPrint`); it's low-value to add a third RTL test, but the branch exists and matches the design.
- **Type consistency:** `PrintService.openForPrint(path: string): Promise<void>` and `PosServices.print` used identically in T1 and T2; `ExportedFile` shape (`path`/`mimeType: "text/html"`) matches the SW-6c type already in `types.ts`.
- **Reuse:** no export command is modified; print buttons call the existing `exportEvidence`/`exportLabels`/`exportCorrectionReport`.
- **No boot:** all new code is a plugin call, a JSON line, and buttons — verified by `bun`/`cargo` gates; the OS open/print is the only inherent gap, per the design.
