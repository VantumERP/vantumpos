# VantumPOS MVP Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the 12 documented gaps in `docs/PROGRESS.md` so every VantumPOS module meets its `docs/module-specs` Acceptance Criteria and the 7-point Definition of Done.

**Architecture:** Local-first Tauri + React + Rust + SQLite POS. Rust owns state-changing validation and transactions; React screens are service-backed through `src/services/ports.ts` + adapters (`local-adapter.ts` real, `mock-adapter.ts` for tests); money/quantities are integer minor (para) / milli units; errors use the `{code, message, details}` contract with Serbian operator-facing messages.

**Tech Stack:** React + TypeScript + shadcn/ui + Vite + vitest (frontend); Rust + Tauri v2 + rusqlite/SQLite + `cargo test` (backend); `bun` as the package runner.

## Global Constraints

- **UI:** Reuse existing shadcn components in `src/components/ui/*` — check them before building any UI. Add new components only via `bunx shadcn@latest add <component>` honoring `components.json`. Apply the `frontend-design` and `vercel-react-best-practices` skills.
- **Backend:** Apply the `tauri-v2` and `rust-best-practices` skills.
- **Money/quantities:** integer minor (para) / milli units only — never floats.
- **Errors:** keep the `{code, message, details}` contract with Serbian operator-facing messages.
- **Local-first:** introduce no fiscalization, Medusa, or cloud dependencies.
- **Model:** run all subagent/workflow work on the Opus model.
- **Per-task Definition of Done:** the spec-named happy + failure tests are written FIRST (TDD), then the implementation makes them pass; all five gates are green before the task's commit:
  - `bun run test`
  - `bun run build`
  - `cd src-tauri && cargo test -- --test-threads=1`
  - `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cd src-tauri && cargo fmt --check`
- **Sequencing:** Task 2 introduces the role/session plumbing (passing the acting user's role to the backend and a `require_admin`-style gate). Task 8 (Reports role-gate) and Task 1 (audit-user threading) consume that plumbing rather than redefining it. Do Tasks 2, 6, and 11 (all touch `settings`) in that order to avoid churn.

---

I now have full ground truth. Here is the task section.

### Task 1: A1 (rec #4) — Thread session user into Inventory & Receipts

**Files:**
- Modify: `/Users/adnan/Projects/vantumpos/src/app/inventory/InventoryScreen.tsx` (props `77-79`, signature `98`, dialog render `274-283`, dialog props type `426-436`, request `463-469`)
- Modify: `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.tsx` (props `63-65`, signature `121`, void request `171-175`, return request `234-239`)
- Modify: `/Users/adnan/Projects/vantumpos/src/app/AppShell.tsx` (render sites `368-374`)
- Create (test): `/Users/adnan/Projects/vantumpos/src/app/inventory/InventoryScreen.test.tsx`
- Create (test): `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.test.tsx`

**Interfaces:**
- Consumes: `session.user.id` from `renderModule({ session, ... })` in `AppShell.tsx:307-381` (already in scope; `session: AppSession`, `AppSession.user.id: number`). Consumes `createMockServices(): PosServices & { initialSession }` from `@/services/mock-adapter` and the existing service ports `services.inventory.receiveStock/writeOffStock(request: InventoryAdjustmentRequest)`, `services.receipts.voidReceipt(request: VoidReceiptRequest)`, `services.receipts.returnItems(request: ReturnItemsRequest)`. No dependency on Task 2.
- Produces: `InventoryScreen({ services, userId }: { services: PosServices; userId: number })` and `ReceiptsScreen({ receipts, userId }: { receipts: ReceiptsService; userId: number })` — both now require a `userId: number` prop. AppShell passes `userId={session.user.id}`. No new commands/types; the existing `InventoryAdjustmentRequest.userId`, `VoidReceiptRequest.userId`, `ReturnItemsRequest.userId` now carry the real operator id instead of the literal `1`.

---

- [ ] **Step 1: Write the failing Inventory test** — create `/Users/adnan/Projects/vantumpos/src/app/inventory/InventoryScreen.test.tsx` (mirrors `RegisterScreen.test.tsx` conventions: explicit vitest imports, `userEvent.setup()`, `createMockServices()` + `vi.spyOn`, Serbian copy assertions):

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { InventoryScreen } from "./InventoryScreen";
import { createMockServices } from "@/services/mock-adapter";

describe("InventoryScreen", () => {
  it("threads the session user id into the inventory movement", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const receiveStock = vi.spyOn(services.inventory, "receiveStock");

    render(<InventoryScreen services={services} userId={7} />);

    await user.click(
      await screen.findByRole("button", { name: "Prijem robe za Mleko 1 l" }),
    );

    const quantity = await screen.findByLabelText("Kolicina");
    await user.clear(quantity);
    await user.type(quantity, "2");
    await user.type(screen.getByLabelText("Razlog"), "Dostava");
    await user.click(screen.getByRole("button", { name: "Sacuvaj prijem" }));

    await waitFor(() =>
      expect(receiveStock).toHaveBeenCalledWith(
        expect.objectContaining({ productId: 1, quantityMilli: 2000, userId: 7 }),
      ),
    );
  });

  it("renders the backend error and still threads the operator on a failed write-off", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const writeOffStock = vi.spyOn(services.inventory, "writeOffStock");

    render(<InventoryScreen services={services} userId={7} />);

    await user.click(
      await screen.findByRole("button", { name: "Otpis za Mleko 1 l" }),
    );

    const quantity = await screen.findByLabelText("Kolicina");
    await user.clear(quantity);
    await user.type(quantity, "5");
    await user.type(screen.getByLabelText("Razlog"), "Lom");
    await user.click(screen.getByRole("button", { name: "Sacuvaj otpis" }));

    expect(await screen.findByText("Nema dovoljno zaliha.")).toBeInTheDocument();
    expect(writeOffStock).toHaveBeenCalledWith(
      expect.objectContaining({ quantityMilli: 5000, userId: 7 }),
    );
  });
});
```

  (Mock seed facts that make this deterministic: product id `1` is `"Mleko 1 l"` with `currentStockMilli: 3000`, `allowNegativeStock: false` — `mock-adapter.ts:40-54`; write-off of `5000` drives stock to `-2000` and throws `{ code: "insufficient_stock", message: "Nema dovoljno zaliha." }` — `mock-adapter.ts:174-176`.)

- [ ] **Step 2: Run it, expect FAIL** — the `userId={7}` prop is currently unknown to the component (esbuild ignores the TS error) and the request still hard-codes `userId: 1`:

```
bunx vitest run src/app/inventory/InventoryScreen.test.tsx
```

  Expected:
```
FAIL  src/app/inventory/InventoryScreen.test.tsx > InventoryScreen > threads the session user id into the inventory movement
AssertionError: expected "receiveStock" to be called with arguments: [ ObjectContaining{"productId": 1, "quantityMilli": 2000, "userId": 7} ]
Received: { "productId": 1, "quantityMilli": 2000, "reason": "Dostava", "userId": 1 }
```

- [ ] **Step 3: Implement Inventory threading** — make these four edits in `/Users/adnan/Projects/vantumpos/src/app/inventory/InventoryScreen.tsx`.

  (a) Props interface (`77-79`):
```tsx
interface InventoryScreenProps {
  services: PosServices;
  userId: number;
}
```
  (b) Component signature (`98`):
```tsx
export function InventoryScreen({ services, userId }: InventoryScreenProps) {
```
  (c) Pass it into the dialog at the render site (`274-283`) — add the `userId` prop:
```tsx
      <InventoryAdjustmentDialog
        adjustment={adjustment}
        services={services}
        userId={userId}
        onOpenChange={(open) => {
          if (!open) {
            setAdjustment(null);
          }
        }}
        onSaved={handleAdjustmentSaved}
      />
```
  (d) Dialog props type + destructure (`426-436`) — add `userId: number;` to the type and `userId` to the destructure:
```tsx
function InventoryAdjustmentDialog({
  adjustment,
  services,
  userId,
  onOpenChange,
  onSaved,
}: {
  adjustment: AdjustmentState | null;
  services: PosServices;
  userId: number;
  onOpenChange: (open: boolean) => void;
  onSaved: (productId: number) => Promise<void>;
}) {
```
  (e) Replace the hard-coded operator in the request (`463-469`):
```tsx
      const quantityMilli = parseQuantityInput(quantity);
      const request: InventoryAdjustmentRequest = {
        productId: adjustment.item.productId,
        quantityMilli,
        reason,
        userId,
      };
```

- [ ] **Step 4: Run it, expect PASS**

```
bunx vitest run src/app/inventory/InventoryScreen.test.tsx
```
  Expected: `Test Files  1 passed (1)` / `Tests  2 passed (2)`.

- [ ] **Step 5: Write the failing Receipts test** — create `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.test.tsx`. Covers void-happy (threads id), return-happy (threads id), and return-failure (client guard skips the service). The void path is asserted on the spy (the `Storniraj racun` confirm is a Radix `AlertDialogAction` that auto-closes, so we assert the call, not the dialog); the return path uses the `Sheet` submit which keeps its error visible.

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReceiptsScreen } from "./ReceiptsScreen";
import { createMockServices } from "@/services/mock-adapter";

describe("ReceiptsScreen", () => {
  it("threads the session user id into the void call", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const voidReceipt = vi.spyOn(services.receipts, "voidReceipt");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Storniraj racun" }));
    await user.type(screen.getByLabelText("Razlog"), "Greska kasira");
    await user.click(screen.getByRole("button", { name: "Potvrdi storniranje" }));

    await waitFor(() =>
      expect(voidReceipt).toHaveBeenCalledWith({
        receiptId: 1,
        userId: 9,
        reason: "Greska kasira",
      }),
    );
  });

  it("threads the session user id into the return call", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));

    const quantity = await screen.findByLabelText("Kolicina za Kafa 200 g");
    await user.clear(quantity);
    await user.type(quantity, "1");
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sacuvaj povrat" }));

    await waitFor(() =>
      expect(returnItems).toHaveBeenCalledWith(
        expect.objectContaining({
          receiptId: 1,
          userId: 9,
          items: [{ saleItemId: 1, quantityMilli: 1000 }],
        }),
      ),
    );
  });

  it("blocks the return and skips the service when no quantity is entered", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sacuvaj povrat" }));

    expect(
      await screen.findByText("Unesite kolicinu za bar jedan artikal."),
    ).toBeInTheDocument();
    expect(returnItems).not.toHaveBeenCalled();
  });
});
```

  (Mock fixture facts: `searchReceipts({})` returns one receipt `"R-2026-0001"` with `canVoid: true`, `canReturn: true`, and a single item id `1` `"Kafa 200 g"` — `mock-adapter.ts:1078-1119`, `588-600`. `parseQuantityInput("1")` from `@/app/format` yields `1000`. The empty-quantity guard `"Unesite kolicinu za bar jedan artikal."` is at `ReceiptsScreen.tsx:229-232`.)

- [ ] **Step 6: Run it, expect FAIL** — void/return requests still send `userId: 1`:

```
bunx vitest run src/app/ReceiptsScreen.test.tsx
```
  Expected:
```
FAIL  src/app/ReceiptsScreen.test.tsx > ReceiptsScreen > threads the session user id into the void call
AssertionError: expected "voidReceipt" to be called with arguments: [ {"receiptId": 1, "userId": 9, "reason": "Greska kasira"} ]
Received: { "receiptId": 1, "userId": 1, "reason": "Greska kasira" }
```
  (The return-failure test already passes — it only asserts the client guard — but the two threading tests fail.)

- [ ] **Step 7: Implement Receipts threading** — edits in `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.tsx`.

  (a) Props interface (`63-65`):
```tsx
interface ReceiptsScreenProps {
  receipts: ReceiptsService;
  userId: number;
}
```
  (b) Component signature (`121`):
```tsx
export function ReceiptsScreen({ receipts, userId }: ReceiptsScreenProps) {
```
  (c) Void request (`171-175`):
```tsx
      const detail = await receipts.voidReceipt({
        receiptId: selected.id,
        userId,
        reason: voidReason,
      });
```
  (d) Return request (`234-239`):
```tsx
      const detail = await receipts.returnItems({
        receiptId: selected.id,
        userId,
        reason: returnReason,
        items,
      });
```

- [ ] **Step 8: Run it, expect PASS**

```
bunx vitest run src/app/ReceiptsScreen.test.tsx
```
  Expected: `Test Files  1 passed (1)` / `Tests  3 passed (3)`.

- [ ] **Step 9: Wire the real session id in AppShell** — in `/Users/adnan/Projects/vantumpos/src/app/AppShell.tsx`, pass `session.user.id` at the two render sites (`368-374`):
```tsx
  if (activeId === "inventory") {
    return <InventoryScreen services={services} userId={session.user.id} />;
  }

  if (activeId === "receipts") {
    return <ReceiptsScreen receipts={services.receipts} userId={session.user.id} />;
  }
```

- [ ] **Step 10: Confirm no `userId: 1` remains and run the full frontend gates** — grep must return nothing, then the whole suite and the type-checked build must pass:
```
grep -rn "userId: 1" src/app && echo "STILL PRESENT" || echo "CLEAN"
bun run test
bun run build
```
  Expected: `CLEAN`; `bun run test` ends with all files passed (including the two new files); `bun run build` completes with no TypeScript errors (the new required `userId` prop is satisfied at every call site in `AppShell.tsx`).

- [ ] **Final step: Commit**
```
git add src/app/inventory/InventoryScreen.tsx src/app/inventory/InventoryScreen.test.tsx \
        src/app/ReceiptsScreen.tsx src/app/ReceiptsScreen.test.tsx src/app/AppShell.tsx
git commit -m "feat: thread session user id into inventory and receipts audit trail

Replace hard-coded userId: 1 in inventory adjustments, receipt voids, and
returns with the logged-in operator (session.user.id) threaded as a userId
prop from AppShell, so movements and void/return documents record the real
acting cashier.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: A2 (rec #2) — Enforce admin role-gating for receipt-sequence updates

**Files:**
- Modify: `src-tauri/src/commands/auth.rs` — add `require_admin` helper after `active_user_by_id` (currently ends at `auth.rs:151`)
- Modify: `src-tauri/src/commands/settings.rs` — enforce in `save_receipt_settings` (`settings.rs:276-290`); update existing test (`settings.rs:484-506`); add test helpers + new tests inside `mod tests` (`settings.rs:381-507`)
- Modify: `src/app/navigation.ts` — `NavigationItem` interface (`navigation.ts:13-17`) + `settings` item (`navigation.ts:50-54`)
- Modify: `src/app/AppShell.tsx` — nav render filter (`AppShell.tsx:220`); `renderModule` settings branch (`AppShell.tsx:355-362`)
- Modify: `src/services/mock-adapter.ts` — `updateReceiptSettings` role gate (`mock-adapter.ts:258-261`)
- Test: `src-tauri/src/commands/settings.rs` (`mod tests`), `src/App.test.tsx` (add to `describe("AppShell", …)`)

No `src-tauri/src/lib.rs` / capabilities change: `settings_update_receipt` is already registered (`lib.rs:62`) and already receives `state: State<'_, AppState>`, which holds the logged-in user id (`state.session_user_id()`). The admin check is derived server-side from that existing session — NOT from a client-supplied role (which would be spoofable) and NOT a new request field, so `ReceiptSettingsRequest`, `ports.ts`, and `local-adapter.ts` are unchanged.

**Interfaces:**
- Consumes (existing): `AppState::session_user_id(&self) -> Result<Option<i64>, AppError>` (`state.rs:27`); `active_user_by_id(state: &AppState, user_id: i64) -> Result<Option<UserAccount>, AppError>` (`auth.rs:136`, already `pub(crate)`); `AppError::business(code: &'static str, message: impl Into<String>) -> AppError` (`app_error.rs:47`); `AppError::code()` test-only (`app_error.rs:67`); `AppState::set_session_user_id(&self, user_id: i64)` (`state.rs:34`); `UserRole = "admin" | "cashier"` (`types.ts`).
- Produces (this task DEFINES the backend role-gate helper; later admin-gated tasks such as B3 Reports CONSUME it): `pub(crate) fn require_admin(state: &AppState) -> Result<UserAccount, AppError>` in `commands/auth.rs` — returns the signed-in admin `UserAccount`, or `AppError::business("unauthorized", "Niste prijavljeni.")` when no/invalid session, or `AppError::business("forbidden", "Samo administrator moze da menja numeraciju racuna.")` for non-admins. Also produces the `NavigationItem.adminOnly?: boolean` field and the role-filtered nav.

---

- [ ] **Step 1: Write the failing Rust tests** — add two test helpers and three tests to `mod tests` in `src-tauri/src/commands/settings.rs`, and sign in admin in the existing round-trip test.

Add these helpers immediately after `with_test`/`with_state` (after `settings.rs:397`, before `company_settings_round_trip_persists_json_value`):

```rust
    fn sign_in_admin(state: &AppState) {
        let admin_id: i64 = state
            .db()
            .open()
            .expect("database should open")
            .query_row(
                "SELECT id FROM users WHERE username = 'admin'",
                [],
                |row| row.get(0),
            )
            .expect("bootstrap admin should exist");
        state
            .set_session_user_id(admin_id)
            .expect("admin session should set");
    }

    fn sign_in_cashier(state: &AppState) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('marko', 'Marko Markovic', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
    }
```

Update the existing happy-path test (`settings.rs:489-496`) to sign in admin first — change the body of `receipt_numbering_round_trip_persists_no_reset_policy` so the `save_receipt_settings(...)` call is preceded by `sign_in_admin(state);`:

```rust
            |state| {
                sign_in_admin(state);

                let saved = save_receipt_settings(
                    state,
                    ReceiptSettingsRequest {
                        prefix: "VP-".to_string(),
                        next_sequence_number: 42,
                    },
                )
                .expect("receipt settings should save");
```

Add the new failure-path + admin-path tests just before the closing `}` of `mod tests` (`settings.rs:507`):

```rust
    #[test]
    fn receipt_numbering_update_allowed_for_admin() {
        with_state("receipt_numbering_update_allowed_for_admin", |state| {
            sign_in_admin(state);

            let saved = save_receipt_settings(
                state,
                ReceiptSettingsRequest {
                    prefix: "VP-".to_string(),
                    next_sequence_number: 7,
                },
            )
            .expect("admin should update receipt numbering");

            assert_eq!(saved.next_sequence_number, 7);
        });
    }

    #[test]
    fn receipt_numbering_update_rejected_for_cashier() {
        with_state("receipt_numbering_update_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let error = save_receipt_settings(
                state,
                ReceiptSettingsRequest {
                    prefix: "VP-".to_string(),
                    next_sequence_number: 7,
                },
            )
            .expect_err("cashier should not update receipt numbering");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn receipt_numbering_update_rejected_without_session() {
        with_state(
            "receipt_numbering_update_rejected_without_session",
            |state| {
                let error = save_receipt_settings(
                    state,
                    ReceiptSettingsRequest {
                        prefix: "VP-".to_string(),
                        next_sequence_number: 7,
                    },
                )
                .expect_err("anonymous caller should not update receipt numbering");

                assert_eq!(error.code(), "unauthorized");
            },
        );
    }
```

- [ ] **Step 2: Run the Rust tests, expect FAIL** (enforcement not yet implemented; `save_receipt_settings` ignores the session and returns `Ok`):

```
cd src-tauri && cargo test receipt_numbering -- --test-threads=1
```

Expected: compiles, then
```
test commands::settings::tests::receipt_numbering_round_trip_persists_no_reset_policy ... ok
test commands::settings::tests::receipt_numbering_update_allowed_for_admin ... ok
test commands::settings::tests::receipt_numbering_update_rejected_for_cashier ... FAILED
test commands::settings::tests::receipt_numbering_update_rejected_without_session ... FAILED
...
test result: FAILED. 2 passed; 2 failed; ...
```
with panics like `cashier should not update receipt numbering: ReceiptSettings { prefix: "VP-", next_sequence_number: 7, reset_policy: "none" }`.

- [ ] **Step 3: Implement the backend role gate.** In `src-tauri/src/commands/auth.rs`, add the helper after `active_user_by_id` (after `auth.rs:151`):

```rust
pub(crate) fn require_admin(state: &AppState) -> Result<UserAccount, AppError> {
    let user_id = state
        .session_user_id()?
        .ok_or_else(|| AppError::business("unauthorized", "Niste prijavljeni."))?;

    let user = active_user_by_id(state, user_id)?
        .ok_or_else(|| AppError::business("unauthorized", "Niste prijavljeni."))?;

    if user.role != "admin" {
        return Err(AppError::business(
            "forbidden",
            "Samo administrator moze da menja numeraciju racuna.",
        ));
    }

    Ok(user)
}
```

In `src-tauri/src/commands/settings.rs`, enforce it as the first line of `save_receipt_settings` (`settings.rs:276-280`):

```rust
pub fn save_receipt_settings(
    state: &AppState,
    request: ReceiptSettingsRequest,
) -> Result<ReceiptSettings, AppError> {
    super::auth::require_admin(state)?;
    validate_receipt_request(&request)?;

    let settings = ReceiptSettings {
        prefix: request.prefix.trim().to_string(),
        next_sequence_number: request.next_sequence_number,
        reset_policy: default_reset_policy(),
    };

    save_json_setting(state, RECEIPT_SETTINGS_KEY, &settings)?;
    Ok(settings)
}
```

- [ ] **Step 4: Run the Rust tests, expect PASS:**

```
cd src-tauri && cargo test receipt_numbering -- --test-threads=1
```

Expected: `test result: ok. 4 passed; 0 failed; ...` (round-trip + admin-allowed succeed because they sign in admin; cashier rejected with `forbidden`; no-session rejected with `unauthorized`).

- [ ] **Step 5: Run the full backend gates** (the helper is reached by the non-test command path, so no dead-code/clippy warning):

```
cd src-tauri && cargo test -- --test-threads=1
cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings
cd src-tauri && cargo fmt --check
```

Expected: all tests pass, clippy clean (`Finished`), `fmt --check` produces no diff.

- [ ] **Step 6: Write the failing frontend test.** In `src/App.test.tsx`, add two tests inside `describe("AppShell", …)` (after the test at `App.test.tsx:122-135`). They reuse the existing `cashierSession` (`App.test.tsx:43-55`), `adminSession` (`App.test.tsx:57-84`), and `buildAuthServices` (`App.test.tsx:18-41`):

```tsx
  it("hides the Podesavanja nav item from cashiers", async () => {
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(cashierSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });

    render(<AppShell services={services} />);

    expect(await screen.findByText("Marko Markovic")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kasa" })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Podesavanja" }),
    ).not.toBeInTheDocument();
  });

  it("shows the Podesavanja nav item for admins", async () => {
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(adminSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });

    render(<AppShell services={services} />);

    expect(
      await screen.findByRole("button", { name: "Podesavanja" }),
    ).toBeInTheDocument();
  });
```

(The cashier case has `currentShift: null`, so `renderModule` shows `OpenShiftScreen` — which renders cleanly with `buildAuthServices` exactly as the existing "routes a cashier without an open shift" test at `App.test.tsx:97-120` proves; the sidebar nav still renders, and the cashier's display name `Marko Markovic` appears in the sidebar footer at `AppShell.tsx:247`. The admin case mirrors the working `App.test.tsx:122-135` test.)

- [ ] **Step 7: Run the frontend test, expect FAIL** (every nav item, including `settings`, renders unconditionally today at `AppShell.tsx:220`):

```
bun run test src/App.test.tsx
```

Expected: `shows the Podesavanja nav item for admins` passes, `hides the Podesavanja nav item from cashiers` FAILS at `expect(screen.queryByRole("button", { name: "Podesavanja" })).not.toBeInTheDocument()` with `received element is in the document`.

- [ ] **Step 8: Implement the nav role gate.** In `src/app/navigation.ts`, add the optional field to the interface (`navigation.ts:13-17`):

```ts
export interface NavigationItem {
  id: string;
  label: string;
  icon: LucideIcon;
  adminOnly?: boolean;
}
```

and mark the `settings` item (`navigation.ts:50-54`):

```ts
  {
    id: "settings",
    label: "Podesavanja",
    icon: SettingsIcon,
    adminOnly: true,
  },
```

In `src/app/AppShell.tsx`, filter the nav render. Replace the opening of the map at `AppShell.tsx:220` (`{navigationItems.map((item) => {`) with a filtered chain (the `"adminOnly" in item` narrowing keeps each `item.id` typed as the literal `NavigationItemId`, so `setActiveId(item.id)` further down still type-checks):

```tsx
                  {navigationItems
                    .filter(
                      (item) =>
                        session.user.role === "admin" ||
                        !("adminOnly" in item && item.adminOnly),
                    )
                    .map((item) => {
```

Add a defense-in-depth content guard in `renderModule`. Replace the settings branch (`AppShell.tsx:355-362`) with (`Alert`, `AlertTitle`, `AlertDescription` are already imported and used at `AppShell.tsx:174`):

```tsx
  if (activeId === "settings") {
    if (session.user.role !== "admin") {
      return (
        <Alert>
          <AlertTitle>Podesavanja</AlertTitle>
          <AlertDescription>
            Samo administrator moze da menja podesavanja.
          </AlertDescription>
        </Alert>
      );
    }

    return (
      <SettingsScreen
        services={services}
        usersPanel={<UsersScreen services={services} currentUser={session.user} />}
      />
    );
  }
```

- [ ] **Step 9: Mirror the gate in the mock adapter** (keeps mock/backend error parity per repo standards; mock default `session` is admin so existing flows are unaffected). In `src/services/mock-adapter.ts`, update `updateReceiptSettings` (`mock-adapter.ts:258-261`):

```ts
      async updateReceiptSettings(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator moze da menja numeraciju racuna.",
          };
        }

        receiptSettings = { ...request, resetPolicy: "none" };
        return receiptSettings;
      },
```

- [ ] **Step 10: Run the frontend test, expect PASS:**

```
bun run test src/App.test.tsx
```

Expected: both new tests pass; existing AppShell tests still pass (admin sessions retain the `Podesavanja` button).

- [ ] **Step 11: Run the full frontend gates:**

```
bun run test
bun run build
```

Expected: full vitest suite green; `tsc`/vite build succeeds (no type error from the `.filter(...).map(...)` chain or the new `adminOnly` field).

- [ ] **Final step: Commit.**

```
git add src-tauri/src/commands/auth.rs src-tauri/src/commands/settings.rs \
  src/app/navigation.ts src/app/AppShell.tsx src/services/mock-adapter.ts src/App.test.tsx
git commit -m "$(cat <<'EOF'
feat(settings): enforce admin role-gating for receipt numbering

Add require_admin session helper and reject non-admin callers of
save_receipt_settings with a Serbian message; hide and content-gate the
Podesavanja nav item by role; mirror the gate in the mock adapter.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

I have all the ground truth needed. The bug is confirmed: `RegisterScreen.tsx:72-76` defines a `PreviewState` with an `"error"` variant, and the preview effect's catch block (`RegisterScreen.tsx:124-131`) sets `setPreviewState({ status: "error", message: errorMessage(error) })`, but the only render-time use of `previewState.status` is the spinner at line 428 — the error `message` is never displayed. Real backend messages: `"Popust ne moze biti veci od iznosa."` (sales.rs:672) and `"Kolicina mora biti veca od nule."` (sales.rs:349).

### Task 3: A3 (rec #5) — Render swallowed preview/validation error in Register

**Files:**
- Modify: `src/app/register/RegisterScreen.tsx` (insert a render block after the existing `message` Alert at lines 291-296; the swallowed error is set at lines 124-131, type defined at lines 72-76)
- Test: `src/app/register/RegisterScreen.test.tsx` (add two `it(...)` blocks inside the existing `describe("RegisterScreen", ...)` ending at line 229; reuse the module-level `createRegisterServices` factory at lines 35-76 and `addProductToCart` helper at lines 150-161)

**Interfaces:**
- Consumes (already present — no Task 2 dependency; this is a pure render fix, no session/role plumbing):
  - The existing preview effect `src/app/register/RegisterScreen.tsx:108-136` which calls `services.sales.createSalePreview(draft)` and, on rejection, stores `setPreviewState({ status: "error", message: errorMessage(error) })`.
  - Port method `SalesService.createSalePreview(request: SaleDraftRequest): Promise<SalePreview>` (`src/services/ports.ts`), mocked in tests via the `sales.createSalePreview` field of the `createRegisterServices()` factory.
  - `PreviewState` union variant `{ status: "error"; message: string }` (`RegisterScreen.tsx:76`).
- Produces: No new exported symbols, props, commands, or functions. Adds a rendered destructive `Alert` (title `"Pregled racuna nije moguc"`) that displays `previewState.message` whenever `previewState.status === "error"`. Cart state (`cart`) is untouched on preview failure, so the cart is preserved. Nothing downstream depends on new identifiers.

TDD steps:

- [ ] **Step 1: Write the failing test (failure path — swallowed preview error renders, cart preserved).** Add this `it` block inside the existing `describe("RegisterScreen", () => { ... })` in `src/app/register/RegisterScreen.test.tsx`, immediately after the `it("keeps the cart intact when backend completion fails", ...)` test (after line 228, before the closing `});` at line 229). It mutates the factory mock's `createSalePreview` to reject with the real backend `CommandError` shape (mirroring `sales.rs:672`), following the "mutate the mock after creation" convention:

```tsx
  it("renders the swallowed preview error and keeps the cart", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    services.sales.createSalePreview = vi.fn().mockRejectedValue({
      code: "validation_error",
      message: "Popust ne moze biti veci od iznosa.",
    });

    render(<RegisterScreen services={services} />);
    await user.type(
      screen.getByRole("searchbox", {
        name: "Skeniraj barkod ili pretrazi artikal",
      }),
      "Mleko{enter}",
    );

    expect(
      await screen.findByText("Popust ne moze biti veci od iznosa."),
    ).toBeInTheDocument();
    expect(screen.getByText("Mleko 1 l")).toBeInTheDocument();
  });
```

(No new imports needed — `render`, `screen`, `userEvent`, `vi`, `describe`, `expect`, `it`, `RegisterScreen`, and `createRegisterServices` are all already imported/defined in this file at lines 1-76. Typing `"Mleko{enter}"` matches the product via the factory's `searchProducts` filter at lines 49-55, calls `addProduct`, which makes `draft.items.length > 0`, firing the preview effect that now rejects.)

- [ ] **Step 2: Run it, expect FAIL.** Command:

```bash
bunx vitest run src/app/register/RegisterScreen.test.tsx
```

Expected: the new test fails because the error message is stored in `previewState` but never rendered. Output contains:

```
 FAIL  src/app/register/RegisterScreen.test.tsx > RegisterScreen > renders the swallowed preview error and keeps the cart
TestingLibraryElementError: Unable to find an element with the text: Popust ne moze biti veci od iznosa.
```

(The other 5 existing tests still pass: `Tests  1 failed | 5 passed (6)`.)

- [ ] **Step 3: Implement — render the error state.** In `src/app/register/RegisterScreen.tsx`, insert a new destructive `Alert` block right after the existing `message` Alert (between the block ending at line 296 and the `<div className="grid ...">` at line 298). The `Alert`, `AlertTitle`, `AlertDescription` imports are already present (line 10), so no import change is needed. Edit:

```tsx
      {message && (
        <Alert variant="destructive">
          <AlertTitle>Prodaja nije zavrsena</AlertTitle>
          <AlertDescription>{message}</AlertDescription>
        </Alert>
      )}

      {previewState.status === "error" && (
        <Alert variant="destructive">
          <AlertTitle>Pregled racuna nije moguc</AlertTitle>
          <AlertDescription>{previewState.message}</AlertDescription>
        </Alert>
      )}

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
```

(TypeScript narrows `previewState` to the `{ status: "error"; message: string }` variant inside the `&&`, so `previewState.message` typechecks. The shadcn `Alert` component renders `role="alert"`, and completion stays blocked because the `Zavrsi prodaju` button is already `disabled={!preview || isCompleting}` at line 505 — `preview` is `undefined` whenever `previewState.status !== "ready"`. The cart is never cleared on preview failure, satisfying the "preserves the cart" acceptance.)

- [ ] **Step 4: Run it, expect PASS.** Command:

```bash
bunx vitest run src/app/register/RegisterScreen.test.tsx
```

Expected: `Tests  6 passed (6)` — the failure-path test now finds `"Popust ne moze biti veci od iznosa."` in the rendered `Alert` and still finds `"Mleko 1 l"` in the cart table.

- [ ] **Step 5: Add the happy-path guard test (successful preview shows NO error alert).** Add this `it` block immediately after the test from Step 1, still inside the `describe`. It reuses the existing `addProductToCart` helper (lines 150-161), whose `createRegisterServices()` resolves `createSalePreview`, so `previewState` reaches `"ready"` and the new error `Alert` must be absent:

```tsx
  it("does not render a preview error when the preview succeeds", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);

    expect(
      screen.queryByText("Pregled racuna nije moguc"),
    ).not.toBeInTheDocument();
  });
```

- [ ] **Step 6: Run the full file, expect PASS.** Command:

```bash
bunx vitest run src/app/register/RegisterScreen.test.tsx
```

Expected: `Tests  7 passed (7)`.

- [ ] **Step 7: Run the project verification gates.** Commands and expected results:

```bash
bun run test
```
Expected: all suites pass (whole-repo `vitest run` green, including the 7 RegisterScreen tests).

```bash
bun run build
```
Expected: TypeScript compile + Vite build succeed with no type errors (confirms the `previewState.message` narrowing and JSX are valid).

(The Rust gates — `cd src-tauri && cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check` — are unaffected by this frontend-only change but should still be run to confirm the gate stays green.)

- [ ] **Final step: Commit.** Stage exactly the two touched files and commit:

```bash
git add src/app/register/RegisterScreen.tsx src/app/register/RegisterScreen.test.tsx
git commit -m "$(cat <<'EOF'
fix(register): render swallowed sale-preview validation error

Surface the previously-defined-but-unrendered previewState "error" variant
in RegisterScreen so an over-limit discount or invalid quantity shows the
backend Serbian message and preserves the cart (DoD point 5 for Register).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

Both files are reverted clean; the harness notes are just re-reads. The plan code is fully validated (5 tests pass, clippy clean, fmt clean, and the deactivated-login test was proven non-vacuous via a temporary guard removal). Here is the section.

### Task 4: A4 (rec #1) — Add missing auth/shift backend tests

This task is **test-only**: A4 adds characterization tests that guard *existing* backend behavior (design spec lines 107-112: "No behavior change expected... expected to pass against existing behavior; if they fail, fix the behavior"). `commands/auth.rs` and `commands/shifts.rs` currently have **no `#[cfg(test)] mod tests`** module. We add one to each, covering the module-spec backend test list (`docs/module-specs/01-auth-shifts.md:134-140`): login success, login failure (invalid credentials + deactivated user), second-open-shift rejection, and close-shift expected-cash derivation.

**Files:**
- Modify (append a `#[cfg(test)] mod tests` after the last `fn` at `src-tauri/src/commands/auth.rs:193-198`): `src-tauri/src/commands/auth.rs`
- Modify (append a `#[cfg(test)] mod tests` after the last `fn` at `src-tauri/src/commands/shifts.rs:293-298`): `src-tauri/src/commands/shifts.rs`

**Interfaces:**
- Consumes (existing, no Task-2 plumbing needed — these are pure-Rust inner functions called directly, not the `#[tauri::command]` wrappers):
  - `pub fn login_user(state: &AppState, request: LoginRequest) -> Result<AuthSession, CommandError>` (`auth.rs:81`); `LoginRequest { username: String, credential: String }` (`auth.rs:25-30`); returns `AuthSession { user: UserAccount, current_shift: Option<ShiftSummary> }` (`auth.rs:32-37`).
  - `pub fn open_shift_for_user(state: &AppState, user_id: i64, request: OpenShiftRequest) -> Result<ShiftSummary, CommandError>` (`shifts.rs:86`); `OpenShiftRequest { opening_cash_minor: i64, note: Option<String> }` (`shifts.rs:28-33`).
  - `pub fn close_shift_for_user(state: &AppState, user_id: i64, request: CloseShiftRequest) -> Result<ShiftSummary, CommandError>` (`shifts.rs:132`); `CloseShiftRequest { shift_id: i64, counted_cash_minor: i64, note: Option<String> }` (`shifts.rs:35-41`); returns `ShiftSummary` with derived `expected_cash_minor`/`cash_sales_minor`/`difference_minor` (`shifts.rs:9-26`, derivation at `shifts.rs:154` + `shifts.rs:262-291`).
  - Test infra: `crate::db::{test_database_path, Db}` (`db/mod.rs:88`, `db/mod.rs:18` — `Db::new` migrates and seeds a bootstrap `admin` user with PIN `1234`); `crate::state::AppState` (`AppState::new(db)` at `state.rs:16`, `state.set_session_user_id`/`session_user_id` at `state.rs:27,34`, `state.db()` at `state.rs:23`); `crate::security::hash_credential(&str) -> Result<String, AppError>` (`security.rs:6`); `CommandError.code: &'static str` and `.message: String` public fields (`app_error.rs:82-86`).
- Produces: no new public API. New test fns only: `commands::auth::tests::{login_user_succeeds_with_valid_credentials, login_user_rejects_invalid_credentials, login_user_rejects_deactivated_user}` and `commands::shifts::tests::{close_shift_for_user_derives_expected_cash, open_shift_for_user_rejects_second_open_shift}`.

---

- [ ] **Step 1: Add the auth test module (happy + two failure paths).** Append to `src-tauri/src/commands/auth.rs`, immediately after the closing `}` of `invalid_credentials_error()` (current last line `auth.rs:198`):

  ```rust
  #[cfg(test)]
  mod tests {
      use rusqlite::params;

      use super::{login_user, LoginRequest};
      use crate::db::{test_database_path, Db};
      use crate::security::hash_credential;
      use crate::state::AppState;

      fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
          let path = test_database_path(test_name);

          {
              let db = Db::new(&path).expect("database should initialize");
              let state = AppState::new(db);
              test(&state);
          }

          std::fs::remove_file(&path).expect("test database should be removed");
      }

      #[test]
      fn login_user_succeeds_with_valid_credentials() {
          with_state("login_succeeds", |state| {
              let session = login_user(
                  state,
                  LoginRequest {
                      username: "admin".to_string(),
                      credential: "1234".to_string(),
                  },
              )
              .expect("seeded admin should log in");

              assert_eq!(session.user.username, "admin");
              assert_eq!(session.user.role, "admin");
              assert!(session.user.active);
              assert!(session.current_shift.is_none());
              assert_eq!(
                  state.session_user_id().expect("session id should read"),
                  Some(session.user.id)
              );
          });
      }

      #[test]
      fn login_user_rejects_invalid_credentials() {
          with_state("login_invalid_credentials", |state| {
              let error = login_user(
                  state,
                  LoginRequest {
                      username: "admin".to_string(),
                      credential: "0000".to_string(),
                  },
              )
              .expect_err("wrong pin should fail");

              assert_eq!(error.code, "invalid_credentials");
              assert!(state
                  .session_user_id()
                  .expect("session id should read")
                  .is_none());
          });
      }

      #[test]
      fn login_user_rejects_deactivated_user() {
          with_state("login_deactivated", |state| {
              let pin_hash = hash_credential("4321").expect("pin hash should compute");
              let connection = state.db().open().expect("database should open");
              connection
                  .execute(
                      "INSERT INTO users (
                          username, display_name, role, pin_hash, active, created_at, updated_at
                       )
                       VALUES ('kasir', 'Kasir', 'cashier', ?1, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                      params![pin_hash],
                  )
                  .expect("deactivated cashier should insert");

              let error = login_user(
                  state,
                  LoginRequest {
                      username: "kasir".to_string(),
                      credential: "4321".to_string(),
                  },
              )
              .expect_err("deactivated user should not log in");

              assert_eq!(error.code, "invalid_credentials");
              assert!(state
                  .session_user_id()
                  .expect("session id should read")
                  .is_none());
          });
      }
  }
  ```

  Note: `with_state` is copied verbatim from the project convention at `commands/backup.rs:378-388`. The deactivated user is given the **correct** PIN (`4321`) so the test proves the `!user.account.active` guard (`auth.rs:93`) — not a wrong PIN — is what blocks login. The `session_user_id().is_none()` assertion additionally guards against a session leak on the failure path.

- [ ] **Step 2: Run the auth tests, expect PASS (characterization green).** From the repo root:

  ```
  cd src-tauri && cargo test commands::auth::tests -- --test-threads=1
  ```

  Expected tail:
  ```
  running 3 tests
  test commands::auth::tests::login_user_rejects_deactivated_user ... ok
  test commands::auth::tests::login_user_rejects_invalid_credentials ... ok
  test commands::auth::tests::login_user_succeeds_with_valid_credentials ... ok

  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 69 filtered out; finished in 1.22s
  ```

- [ ] **Step 3 (non-vacuity RED check for the A4 headline test): temporarily prove `login_user_rejects_deactivated_user` fails if the guard is removed.** In `src-tauri/src/commands/auth.rs`, edit `auth.rs:93` from:

  ```rust
      if !user.account.active || !credential_matches(&user, credential) {
  ```
  to (temporarily):
  ```rust
      if !credential_matches(&user, credential) {
  ```

  Run:
  ```
  cd src-tauri && cargo test commands::auth::tests::login_user_rejects_deactivated_user -- --test-threads=1
  ```

  Expected: FAIL — with the early active-guard gone, `login_user` still ultimately errors at the `active = 1` re-fetch (`active_user_by_id`, `auth.rs:110-112`) but only *after* `set_session_user_id` ran (`auth.rs:106-108`), so the session leaks:
  ```
  test commands::auth::tests::login_user_rejects_deactivated_user ... FAILED
  ...
  assertion failed: state.session_user_id().expect("session id should read").is_none()
  ...
  test result: FAILED. 0 passed; 1 failed; ...
  ```

  Then **revert** `auth.rs:93` back to `if !user.account.active || !credential_matches(&user, credential) {` and re-run Step 2 to confirm 3 passed again. (This is verification only; the real-tree code keeps the guard.)

- [ ] **Step 4: Add the shifts test module (expected-cash derivation + second-open-shift rejection).** Append to `src-tauri/src/commands/shifts.rs`, immediately after the closing `}` of `normalized_note()` (current last line `shifts.rs:298`):

  ```rust
  #[cfg(test)]
  mod tests {
      use rusqlite::params;

      use super::{close_shift_for_user, open_shift_for_user, CloseShiftRequest, OpenShiftRequest};
      use crate::db::{test_database_path, Db};
      use crate::state::AppState;

      fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
          let path = test_database_path(test_name);

          {
              let db = Db::new(&path).expect("database should initialize");
              let state = AppState::new(db);
              test(&state);
          }

          std::fs::remove_file(&path).expect("test database should be removed");
      }

      fn seed_cashier(state: &AppState) -> i64 {
          let connection = state.db().open().expect("database should open");
          connection
              .execute(
                  "INSERT INTO users (username, display_name, role, active, created_at, updated_at)
                   VALUES ('kasir', 'Kasir', 'cashier', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                  [],
              )
              .expect("cashier should insert");
          connection.last_insert_rowid()
      }

      fn seed_completed_sale(
          state: &AppState,
          shift_id: i64,
          cashier_id: i64,
          cash_minor: i64,
          card_minor: i64,
      ) {
          let connection = state.db().open().expect("database should open");
          connection
              .execute(
                  "INSERT INTO sales (
                      local_receipt_number, shift_id, cashier_id, status,
                      subtotal_minor, tax_minor, total_minor, created_at, updated_at
                   )
                   VALUES ('VP-000001', ?1, ?2, 'completed', 20000, 0, 20000,
                           '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                  params![shift_id, cashier_id],
              )
              .expect("sale should insert");
          let sale_id = connection.last_insert_rowid();

          connection
              .execute(
                  "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                   VALUES (?1, 'cash', ?2, '2026-01-01T00:00:00Z')",
                  params![sale_id, cash_minor],
              )
              .expect("cash payment should insert");
          connection
              .execute(
                  "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                   VALUES (?1, 'card', ?2, '2026-01-01T00:00:00Z')",
                  params![sale_id, card_minor],
              )
              .expect("card payment should insert");
      }

      #[test]
      fn close_shift_for_user_derives_expected_cash() {
          with_state("close_shift_derives_expected_cash", |state| {
              let cashier_id = seed_cashier(state);

              let opened = open_shift_for_user(
                  state,
                  cashier_id,
                  OpenShiftRequest {
                      opening_cash_minor: 5000,
                      note: None,
                  },
              )
              .expect("shift should open");
              assert_eq!(opened.status, "open");

              seed_completed_sale(state, opened.id, cashier_id, 12000, 8000);

              let closed = close_shift_for_user(
                  state,
                  cashier_id,
                  CloseShiftRequest {
                      shift_id: opened.id,
                      counted_cash_minor: 17000,
                      note: None,
                  },
              )
              .expect("shift should close");

              assert_eq!(closed.status, "closed");
              assert_eq!(closed.cash_sales_minor, 12000);
              assert_eq!(closed.card_sales_minor, 8000);
              assert_eq!(closed.expected_cash_minor, 17000);
              assert_eq!(closed.counted_cash_minor, Some(17000));
              assert_eq!(closed.difference_minor, Some(0));
          });
      }

      #[test]
      fn open_shift_for_user_rejects_second_open_shift() {
          with_state("open_second_shift_fails", |state| {
              let cashier_id = seed_cashier(state);

              open_shift_for_user(
                  state,
                  cashier_id,
                  OpenShiftRequest {
                      opening_cash_minor: 0,
                      note: None,
                  },
              )
              .expect("first shift should open");

              let error = open_shift_for_user(
                  state,
                  cashier_id,
                  OpenShiftRequest {
                      opening_cash_minor: 0,
                      note: None,
                  },
              )
              .expect_err("second shift should fail");

              assert_eq!(error.code, "validation_error");
              assert_eq!(error.message, "Korisnik vec ima otvorenu smenu.");
          });
      }
  }
  ```

  Money is integer minor units throughout: opening cash `5000`, cash payment `12000`, card payment `8000`; derived `expected_cash_minor = opening (5000) + cash_sales (12000) = 17000` (matches `shifts.rs:154`), and `difference_minor = counted (17000) - expected (17000) = 0` (matches `shifts.rs:273`). The `sales` insert omits `discount_minor`/`fiscal_status`/`document_type` because each has a column DEFAULT (`migrations.rs:108,105,234`); `total_minor`/`subtotal_minor` are set to satisfy the `>= 0` CHECK constraints (`migrations.rs:107-110`). The failure path asserts the exact Serbian operator message from `shifts.rs:101`.

- [ ] **Step 5: Run the shifts tests, expect PASS.**

  ```
  cd src-tauri && cargo test commands::shifts::tests -- --test-threads=1
  ```

  Expected tail:
  ```
  running 2 tests
  test commands::shifts::tests::close_shift_for_user_derives_expected_cash ... ok
  test commands::shifts::tests::open_shift_for_user_rejects_second_open_shift ... ok

  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 70 filtered out; finished in 0.40s
  ```

- [ ] **Step 6: Run the full backend gate (all three tool gates).** Run from the repo root:

  ```
  cd src-tauri && cargo test -- --test-threads=1
  cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings
  cd src-tauri && cargo fmt --check
  ```

  Expected: all suites pass (existing 71 tests + the 5 new tests, 76 total in `vantumpos_lib`); clippy finishes with no warnings; `cargo fmt --check` exits 0 (the test code above is already rustfmt-clean — note the single-line `use super::{...}` import in the shifts module, which rustfmt requires). The frontend gates (`bun run test`, `bun run build`) are unaffected by this Rust-only task but should still be green.

- [ ] **Final step: Commit.**

  ```
  git add src-tauri/src/commands/auth.rs src-tauri/src/commands/shifts.rs
  git commit -m "test: cover auth login + shift close/derive backend paths

  Add the spec-required Rust tests (docs/module-specs/01-auth-shifts.md:134-140):
  login success, invalid-credential and deactivated-user rejection in
  commands/auth.rs; expected-cash derivation on close and second-open-shift
  rejection in commands/shifts.rs. No behavior change.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

I have all the ground truth confirmed. Here is the task section.

---

### Task 5: A5 (rec #12) — Reconcile sales/inventory write divergence

**Files:**
- Modify: `src-tauri/src/commands/inventory.rs` — add `StockMovementWrite`/`StockWriteOutcome` structs + `write_stock_movement` helper (insert before `apply_inventory_adjustment` at line 306); refactor `apply_inventory_adjustment` body (lines 321-393) to call the helper.
- Modify: `src-tauri/src/commands/sales.rs` — add `use crate::commands::inventory::{write_stock_movement, StockMovementWrite};` (after line 7); replace the per-line movement insert + balance upsert (lines 261-289) with a `write_stock_movement` call.
- Test: `src-tauri/src/commands/inventory.rs` tests module — extend `use` block (lines 559-562) and add two tests (`write_stock_movement_sets_absolute_balance`, `write_stock_movement_blocks_negative_for_non_negative_product`).
- Test: `src-tauri/src/commands/sales.rs` tests module — add `complete_sale_transaction_decrements_repeated_product_lines_consistently` (after the existing `complete_sale_transaction_rolls_back_when_stock_is_insufficient` test ending at line 986) as a refactor guard.

**Interfaces:**
- Consumes (existing, in `commands/inventory.rs`): `fn quantity_overflow_error() -> AppError` (line 544), `AppError::not_found(&str)`, `AppError::business(code, msg)`. From `commands/sales.rs`: `complete_sale_transaction(db: &Db, request: CompleteSaleRequest)` (line 180), `OpenShift { id, cashier_id, cashier_name }`, `ComputedLine { product, quantity_milli, .. }`. This task does NOT need the role/session helper from Task 2.
- Produces (new, `pub(crate)` in `commands/inventory.rs`, consumed by `commands/sales.rs` and any future stock-writing path):
  - `pub(crate) struct StockMovementWrite<'a> { pub product_id: i64, pub movement_type: &'a str, pub quantity_milli: i64, pub reason: Option<&'a str>, pub reference_type: Option<&'a str>, pub reference_id: Option<i64>, pub user_id: Option<i64>, pub created_at: &'a str }`
  - `pub(crate) struct StockWriteOutcome { pub movement_id: i64, pub previous_quantity_milli: i64, pub new_quantity_milli: i64 }`
  - `pub(crate) fn write_stock_movement(tx: &Connection, write: StockMovementWrite<'_>) -> Result<StockWriteOutcome, AppError>` — the single source of truth for `inventory_movements` insert + `inventory_balances` absolute-set upsert + the negative-stock guard (`insufficient_stock` / "Nema dovoljno zaliha.").

---

- [ ] **Step 1: Write the failing Rust tests (the new shared helper, happy + failure)**

  In `src-tauri/src/commands/inventory.rs`, extend the tests-module import block (currently lines 559-562) to pull in the not-yet-existing helper:

  ```rust
      use crate::commands::inventory::{
          apply_inventory_adjustment, get_product_ledger_for_connection, list_stock_for_connection,
          write_stock_movement, InventoryAdjustmentRequest, InventoryMovementType, StockListQuery,
          StockMovementWrite,
      };
  ```

  Then add these two tests inside the same `#[cfg(test)] mod tests` block (e.g. right after `receive_stock_creates_movement_and_increases_balance`, which ends at line 705). `with_connection`, `seed_required_data`, `current_balance`, and `use crate::app_error::CommandError;` already exist in this module (lines 565, 583, 657, 558):

  ```rust
      #[test]
      fn write_stock_movement_sets_absolute_balance() {
          with_connection("write_stock_movement_sets_absolute_balance", |connection| {
              connection
                  .execute(
                      "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                       VALUES (1, 5000, '2026-06-18T10:00:00Z')",
                      [],
                  )
                  .expect("balance should seed");

              let outcome = write_stock_movement(
                  connection,
                  StockMovementWrite {
                      product_id: 1,
                      movement_type: "sale",
                      quantity_milli: -2000,
                      reason: Some("Prodaja"),
                      reference_type: Some("sale"),
                      reference_id: Some(7),
                      user_id: Some(1),
                      created_at: "2026-06-18T12:00:00Z",
                  },
              )
              .expect("write should succeed");

              assert_eq!(outcome.previous_quantity_milli, 5000);
              assert_eq!(outcome.new_quantity_milli, 3000);
              assert_eq!(current_balance(connection), 3000);

              let movement: (String, i64, Option<String>, Option<i64>) = connection
                  .query_row(
                      "SELECT movement_type, quantity_milli, reason, user_id
                       FROM inventory_movements WHERE id = ?1",
                      params![outcome.movement_id],
                      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                  )
                  .expect("movement should query");
              assert_eq!(
                  movement,
                  ("sale".to_string(), -2000, Some("Prodaja".to_string()), Some(1))
              );
          });
      }

      #[test]
      fn write_stock_movement_blocks_negative_for_non_negative_product() {
          with_connection(
              "write_stock_movement_blocks_negative_for_non_negative_product",
              |connection| {
                  let error = write_stock_movement(
                      connection,
                      StockMovementWrite {
                          product_id: 1,
                          movement_type: "sale",
                          quantity_milli: -1000,
                          reason: Some("Prodaja"),
                          reference_type: Some("sale"),
                          reference_id: Some(42),
                          user_id: Some(1),
                          created_at: "2026-06-18T12:00:00Z",
                      },
                  )
                  .expect_err("oversell should fail");
                  let command_error = CommandError::from(error);
                  assert_eq!(command_error.code, "insufficient_stock");

                  let movement_count: i64 = connection
                      .query_row(
                          "SELECT COUNT(*) FROM inventory_movements WHERE product_id = 1",
                          [],
                          |row| row.get(0),
                      )
                      .expect("movement count should query");
                  assert_eq!(movement_count, 0);
              },
          );
      }
  ```

  (Seed product id 1 has `allow_negative_stock = 0` per `seed_required_data` at inventory.rs:636, and no `inventory_balances` row, so the failure test sells against a COALESCE-0 balance.)

- [ ] **Step 2: Run it, expect FAIL (unresolved import)**

  ```
  cd src-tauri && cargo test -- --test-threads=1
  ```
  Expected: compile failure before any test runs —
  ```
  error[E0432]: unresolved imports `crate::commands::inventory::write_stock_movement`, `crate::commands::inventory::StockMovementWrite`
   --> src/commands/inventory.rs:561:9
  ```

- [ ] **Step 3: Implement the shared helper and refactor `apply_inventory_adjustment`**

  In `src-tauri/src/commands/inventory.rs`, insert the structs + helper immediately before `pub fn apply_inventory_adjustment` (line 306):

  ```rust
  #[derive(Clone, Copy, Debug)]
  pub(crate) struct StockMovementWrite<'a> {
      pub product_id: i64,
      pub movement_type: &'a str,
      pub quantity_milli: i64,
      pub reason: Option<&'a str>,
      pub reference_type: Option<&'a str>,
      pub reference_id: Option<i64>,
      pub user_id: Option<i64>,
      pub created_at: &'a str,
  }

  #[derive(Clone, Copy, Debug)]
  pub(crate) struct StockWriteOutcome {
      pub movement_id: i64,
      pub previous_quantity_milli: i64,
      pub new_quantity_milli: i64,
  }

  /// Single source of truth for `inventory_movements` + `inventory_balances`
  /// writes, shared by the inventory-adjustment path and the sales path so the
  /// two cannot drift in balance math or negative-stock rules. The caller must
  /// supply an already-open transaction; this function does not commit.
  pub(crate) fn write_stock_movement(
      tx: &Connection,
      write: StockMovementWrite<'_>,
  ) -> Result<StockWriteOutcome, AppError> {
      let (previous_quantity_milli, allow_negative_stock): (i64, bool) = tx
          .query_row(
              r#"
  SELECT COALESCE(ib.quantity_milli, 0), p.allow_negative_stock = 1
  FROM products p
  LEFT JOIN inventory_balances ib ON ib.product_id = p.id
  WHERE p.id = ?1
  "#,
              params![write.product_id],
              |row| Ok((row.get(0)?, row.get(1)?)),
          )
          .optional()?
          .ok_or_else(|| AppError::not_found("Artikal nije pronadjen."))?;

      let new_quantity_milli = previous_quantity_milli
          .checked_add(write.quantity_milli)
          .ok_or_else(quantity_overflow_error)?;

      if new_quantity_milli < 0 && !allow_negative_stock {
          return Err(AppError::business(
              "insufficient_stock",
              "Nema dovoljno zaliha.",
          ));
      }

      tx.execute(
          r#"
  INSERT INTO inventory_movements (
      product_id,
      movement_type,
      quantity_milli,
      reason,
      reference_type,
      reference_id,
      user_id,
      created_at
  )
  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
  "#,
          params![
              write.product_id,
              write.movement_type,
              write.quantity_milli,
              write.reason,
              write.reference_type,
              write.reference_id,
              write.user_id,
              write.created_at,
          ],
      )?;
      let movement_id = tx.last_insert_rowid();

      tx.execute(
          r#"
  INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
  VALUES (?1, ?2, ?3)
  ON CONFLICT(product_id) DO UPDATE SET
      quantity_milli = excluded.quantity_milli,
      updated_at = excluded.updated_at
  "#,
          params![write.product_id, new_quantity_milli, write.created_at],
      )?;

      Ok(StockWriteOutcome {
          movement_id,
          previous_quantity_milli,
          new_quantity_milli,
      })
  }
  ```

  Then replace the body of `apply_inventory_adjustment` from the `let tx = connection.transaction()?;` line through `tx.commit()?;` (current lines 321-393) so the balance read, guard, movement insert, and balance upsert are all delegated to the helper. The new body, keeping the existing `purchase_price_minor` update, becomes:

  ```rust
      let tx = connection.transaction()?;

      if let Some(purchase_price_minor) = request.purchase_price_minor {
          tx.execute(
              "UPDATE products
               SET purchase_price_minor = ?1, updated_at = ?2
               WHERE id = ?3",
              params![purchase_price_minor, created_at, request.product_id],
          )?;
      }

      let outcome = write_stock_movement(
          &tx,
          StockMovementWrite {
              product_id: request.product_id,
              movement_type: movement_type.as_str(),
              quantity_milli: delta_quantity_milli,
              reason: reason.as_deref(),
              reference_type: reference_type.as_deref(),
              reference_id: request.reference_id,
              user_id: request.user_id,
              created_at,
          },
      )?;

      tx.commit()?;

      Ok(InventoryAdjustmentResult {
          product_id: request.product_id,
          movement_id: outcome.movement_id,
          movement_type: movement_type.as_str().to_string(),
          quantity_milli: delta_quantity_milli,
          previous_quantity_milli: outcome.previous_quantity_milli,
          new_quantity_milli: outcome.new_quantity_milli,
          created_at: created_at.to_string(),
      })
  ```

  (The `delta_quantity_milli`, `reason`, and `reference_type` locals computed at lines 314-320 are retained unchanged. The earlier balance read at 322-344 is removed since the helper now owns it; behavior is identical because everything runs inside the same `tx` that rolls back on `Err`.)

- [ ] **Step 4: Run it, expect PASS (helper tests + existing inventory tests green)**

  ```
  cd src-tauri && cargo test -- --test-threads=1
  ```
  Expected: the two new tests pass, and the pre-existing `receive_stock_creates_movement_and_increases_balance`, `write_off_rejects_negative_result_and_rolls_back_balance`, and `ledger_returns_newest_movements_with_resulting_balance` still pass (regression proof the refactor preserved inventory behavior). No `error[...]` lines; summary shows `test result: ok.`

- [ ] **Step 5: Route the sales write path through the shared helper**

  In `src-tauri/src/commands/sales.rs`, add the import after line 7 (`use crate::app_error::{AppError, CommandError};`):

  ```rust
  use crate::commands::inventory::{write_stock_movement, StockMovementWrite};
  ```

  Replace the per-line inventory writes (current lines 261-289 — the `let movement_quantity = -line.quantity_milli;` plus the two `tx.execute(...)` blocks for `inventory_movements` and `inventory_balances`) with a single helper call inside the `for line in &computation.lines` loop:

  ```rust
          write_stock_movement(
              &tx,
              StockMovementWrite {
                  product_id: line.product.id,
                  movement_type: "sale",
                  quantity_milli: -line.quantity_milli,
                  reason: Some("Prodaja"),
                  reference_type: Some("sale"),
                  reference_id: Some(sale_id),
                  user_id: Some(shift.cashier_id),
                  created_at: &created_at,
              },
          )?;
  ```

  The up-front aggregate `validate_stock(&computation.lines)?` call (sales.rs:197) is intentionally kept — it still fires first with the richer `insufficient_stock` payload (`{ productId, currentStockMilli, requiredQuantityMilli }`), and `write_stock_movement` is now the consistent backstop performing the actual write with the same rule and the same absolute-set balance math as the inventory path.

- [ ] **Step 6: Add the sales refactor-guard test (multi-line same product, absolute-set consistency)**

  In `src-tauri/src/commands/sales.rs`, add this test inside `#[cfg(test)] mod tests` after `complete_sale_transaction_rolls_back_when_stock_is_insufficient` (ends at line 986). `SaleDraftItem`, `CompleteSaleRequest`, `DiscountRequest`, `PaymentDraft`, `PaymentMethod` are already imported (lines 769-772); `params` is imported at line 767:

  ```rust
      #[test]
      fn complete_sale_transaction_decrements_repeated_product_lines_consistently() {
          let seeded = seed_sale_data(5000, true);
          let request = CompleteSaleRequest {
              items: vec![
                  SaleDraftItem {
                      product_id: seeded.product_id,
                      quantity_milli: 2000,
                      discount: None,
                  },
                  SaleDraftItem {
                      product_id: seeded.product_id,
                      quantity_milli: 1000,
                      discount: None,
                  },
              ],
              receipt_discount: Some(DiscountRequest::Amount { amount_minor: 0 }),
              payments: vec![PaymentDraft {
                  method: PaymentMethod::Cash,
                  amount_minor: 40000,
              }],
          };

          complete_sale_transaction(&seeded.db, request).expect("sale should complete");

          let connection = seeded.db.open().expect("database should open");
          let balance: i64 = connection
              .query_row(
                  "SELECT quantity_milli FROM inventory_balances WHERE product_id = ?1",
                  params![seeded.product_id],
                  |row| row.get(0),
              )
              .expect("balance should query");
          assert_eq!(balance, 2000);

          let movement_count: i64 = connection
              .query_row(
                  "SELECT COUNT(*) FROM inventory_movements WHERE product_id = ?1",
                  params![seeded.product_id],
                  |row| row.get(0),
              )
              .expect("movement count should query");
          assert_eq!(movement_count, 2);

          let _ = std::fs::remove_file(seeded.db_path);
      }
  ```

  (Seeded product is priced 12000 minor/unit, so 3 units = 36000 total; cash 40000 is accepted with change. Two lines of the same product apply the absolute-set helper twice within one tx: 5000 → 3000 → 2000, the exact consistency the old SQL-increment-vs-absolute-set split risked.)

- [ ] **Step 7: Run the full Rust suite, expect PASS**

  ```
  cd src-tauri && cargo test -- --test-threads=1
  ```
  Expected: the new sales test passes; existing `complete_sale_transaction_inserts_sale_and_decrements_stock` (balance 5000→3000) and `complete_sale_transaction_rolls_back_when_stock_is_insufficient` (insufficient_stock, sale_count 0, balance 1000) stay green — proving the sales refactor is behavior-preserving. Summary: `test result: ok.`

- [ ] **Step 8: Run the backend gates (clippy + fmt), expect PASS**

  ```
  cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings && cargo fmt --check
  ```
  Expected: no warnings/errors. (The `StockMovementWrite` struct param keeps `write_stock_movement` at two args, avoiding `clippy::too_many_arguments`; `pub(crate)` items are live via both callers so no `dead_code`. If `cargo fmt --check` reports the new tests-module `use` ordering, run `cargo fmt` once and re-run `--check`.)

- [ ] **Step 9: Run the frontend gates (unchanged, must stay green)**

  ```
  bun run test && bun run build
  ```
  Expected: pass with no changes — this task is backend-only; no TS/adapter/mock edits are required because the sales and inventory command signatures and DTOs are unchanged.

- [ ] **Final step: Commit**

  ```
  git add src-tauri/src/commands/inventory.rs src-tauri/src/commands/sales.rs
  git commit -m "$(cat <<'EOF'
  refactor(inventory): unify sales/inventory stock-write path via shared helper

  Extract write_stock_movement (single source of truth for inventory_movements
  insert + absolute-set inventory_balances upsert + negative-stock guard) and
  route both apply_inventory_adjustment and complete_sale_transaction through it,
  so the two paths can no longer drift in balance math or stock rules (A5/rec #12).

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

I have all the ground truth confirmed. The backend `save_tax_rate` (settings.rs:230-270) already supports update + deactivate via `id: Some(...)` and returns `not_found` "PDV stopa nije pronadjena." when no row matches; the SettingsScreen `onSave` container callback (lines 197-212) already merges create-vs-update by id. The only real gap is the UI: `TaxRateDialog` hard-codes `id: null` and resets to blank, and `TaxRatesPanel` has no row edit/deactivate action. Here is the task section.

---

### Task 6: B1 (rec #3) — VAT rate edit/deactivate

**Files:**
- Modify: `src/app/settings/SettingsScreen.tsx` — `TaxRatesPanel` (lines 441-505: add `editingRate` state, "Akcije" column with Uredi/Deaktiviraj row actions, pass `taxRate` prop) and `TaxRateDialog` (lines 507-608: accept `taxRate: TaxRate | null`, prefill on open, submit with the rate's `id`).
- Create (test): `src/app/settings/SettingsScreen.test.tsx` — new frontend test file (none exists today).
- Modify (test): `src-tauri/src/commands/settings.rs` — append two regression tests inside `mod tests` (before the closing `}` at line 507).
- No backend implementation change: `save_tax_rate` (settings.rs:230-270) already supports update + deactivate + `not_found`.

**Interfaces:**
- Consumes (already present, do NOT re-create):
  - Backend `pub fn save_tax_rate(state: &AppState, request: SaveTaxRateRequest) -> Result<TaxRate, AppError>` (settings.rs:230); on `request.id = Some(id)` it `UPDATE`s and returns `AppError::not_found("PDV stopa nije pronadjena.")` when `changed == 0` (settings.rs:248-250).
  - Port `saveTaxRate(request: SaveTaxRateRequest): Promise<TaxRate>` (`src/services/ports.ts:68`); type `SaveTaxRateRequest { id: number | null; name: string; rateBasisPoints: number; active: boolean }` (`src/services/types.ts:39-44`).
  - Container `onSave` in `SettingsScreen` (lines 197-212) already merges by `saved.id` (update existing row vs. prepend new) and toasts "PDV stopa je sacuvana."
  - This task does NOT need Task 2's role helper (B1 acceptance is "deactivate workflow reachable"; no role gate in scope).
- Produces (consumed by no later task; UI-only):
  - `TaxRateDialog` gains a `taxRate: TaxRate | null` prop (edit vs. create).
  - `TaxRatesPanel` row actions with stable accessible names `Uredi ${rate.name}` and `Deaktiviraj ${rate.name}`.

---

- [ ] **Step 1: Write the failing frontend test** — create `src/app/settings/SettingsScreen.test.tsx`:

```tsx
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsScreen } from "./SettingsScreen";
import { createMockServices } from "@/services/mock-adapter";

describe("SettingsScreen VAT rates", () => {
  it("edits an existing VAT rate and threads its id to saveTaxRate", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Uredi PDV 20" }));

    const dialog = await screen.findByRole("dialog", { name: "PDV stopa" });
    const nameInput = within(dialog).getByLabelText("Naziv");
    expect(nameInput).toHaveValue("PDV 20");

    await user.clear(nameInput);
    await user.type(nameInput, "PDV 20 standard");
    await user.click(within(dialog).getByRole("button", { name: "Sacuvaj PDV stopu" }));

    await waitFor(() =>
      expect(saveTaxRate).toHaveBeenCalledWith({
        id: 1,
        name: "PDV 20 standard",
        rateBasisPoints: 2000,
        active: true,
      }),
    );
    expect(await screen.findByText("PDV 20 standard")).toBeInTheDocument();
  });

  it("deactivates a VAT rate instead of deleting it", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Deaktiviraj PDV 20" }));

    await waitFor(() =>
      expect(saveTaxRate).toHaveBeenCalledWith({
        id: 1,
        name: "PDV 20",
        rateBasisPoints: 2000,
        active: false,
      }),
    );

    const row = (await screen.findByText("PDV 20")).closest("tr");
    expect(row).not.toBeNull();
    expect(within(row as HTMLElement).getByText("Neaktivna")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Deaktiviraj PDV 20" }),
    ).not.toBeInTheDocument();
  });

  it("blocks saving a VAT rate with an empty name", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Uredi PDV 20" }));

    const dialog = await screen.findByRole("dialog", { name: "PDV stopa" });
    await user.clear(within(dialog).getByLabelText("Naziv"));
    await user.click(within(dialog).getByRole("button", { name: "Sacuvaj PDV stopu" }));

    expect(
      await within(dialog).findByText("Naziv PDV stope je obavezan."),
    ).toBeInTheDocument();
    expect(saveTaxRate).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run it, expect FAIL** — `bunx vitest run src/app/settings/SettingsScreen.test.tsx`
  Expected: all three fail; first error `TestingLibraryElementError: Unable to find an accessible element with the role "button" and name "Uredi PDV 20"` (the row action does not exist yet).

- [ ] **Step 3: Implement — add the `editingRate` state in `TaxRatesPanel`.** Edit `src/app/settings/SettingsScreen.tsx` line 453:

old:
```tsx
  const [dialogOpen, setDialogOpen] = useState(false);
```
new:
```tsx
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingRate, setEditingRate] = useState<TaxRate | null>(null);
```

- [ ] **Step 4: Implement — reset `editingRate` on the "Nova PDV stopa" button** (lines 465-468):

old:
```tsx
          <Button type="button" onClick={() => setDialogOpen(true)}>
            <PlusIcon data-icon="inline-start" />
            Nova PDV stopa
          </Button>
```
new:
```tsx
          <Button
            type="button"
            onClick={() => {
              setEditingRate(null);
              setDialogOpen(true);
            }}
          >
            <PlusIcon data-icon="inline-start" />
            Nova PDV stopa
          </Button>
```

- [ ] **Step 5: Implement — add the "Akcije" column header** (lines 475-477):

old:
```tsx
              <TableHead>Naziv</TableHead>
              <TableHead>Stopa</TableHead>
              <TableHead>Status</TableHead>
```
new:
```tsx
              <TableHead>Naziv</TableHead>
              <TableHead>Stopa</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Akcije</TableHead>
```

- [ ] **Step 6: Implement — add the row action cell** (lines 482-490):

old:
```tsx
              <TableRow key={rate.id}>
                <TableCell>{rate.name}</TableCell>
                <TableCell>{formatBasisPoints(rate.rateBasisPoints)}</TableCell>
                <TableCell>
                  <Badge variant={rate.active ? "secondary" : "outline"}>
                    {rate.active ? "Aktivna" : "Neaktivna"}
                  </Badge>
                </TableCell>
              </TableRow>
```
new:
```tsx
              <TableRow key={rate.id}>
                <TableCell>{rate.name}</TableCell>
                <TableCell>{formatBasisPoints(rate.rateBasisPoints)}</TableCell>
                <TableCell>
                  <Badge variant={rate.active ? "secondary" : "outline"}>
                    {rate.active ? "Aktivna" : "Neaktivna"}
                  </Badge>
                </TableCell>
                <TableCell className="text-right">
                  <div className="flex justify-end gap-2">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      aria-label={`Uredi ${rate.name}`}
                      onClick={() => {
                        setEditingRate(rate);
                        setDialogOpen(true);
                      }}
                    >
                      Uredi
                    </Button>
                    {rate.active ? (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        aria-label={`Deaktiviraj ${rate.name}`}
                        onClick={() =>
                          void onSave({
                            id: rate.id,
                            name: rate.name,
                            rateBasisPoints: rate.rateBasisPoints,
                            active: false,
                          })
                        }
                      >
                        Deaktiviraj
                      </Button>
                    ) : null}
                  </div>
                </TableCell>
              </TableRow>
```
(Deactivate calls the container `onSave` directly with `active: false` — an UPDATE, never a delete — mirroring the optimistic deactivate pattern in `AppShell.tsx:963-965`. `Button`'s `variant="ghost" size="sm"` matches the catalog "Lager" action at `CatalogModule.tsx:978-987`.)

- [ ] **Step 7: Implement — pass `taxRate` into the dialog** (lines 495-501):

old:
```tsx
      <TaxRateDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onSave={async (request) => {
          await onSave(request);
          setDialogOpen(false);
        }}
      />
```
new:
```tsx
      <TaxRateDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        taxRate={editingRate}
        onSave={async (request) => {
          await onSave(request);
          setDialogOpen(false);
        }}
      />
```

- [ ] **Step 8: Implement — `TaxRateDialog` accepts `taxRate`, prefills, submits its id.** Edit the dialog signature (lines 507-520):

old:
```tsx
function TaxRateDialog({
  open,
  onOpenChange,
  onSave,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (request: {
    id: number | null;
    name: string;
    rateBasisPoints: number;
    active: boolean;
  }) => Promise<void>;
}) {
```
new:
```tsx
function TaxRateDialog({
  open,
  onOpenChange,
  onSave,
  taxRate,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  taxRate: TaxRate | null;
  onSave: (request: {
    id: number | null;
    name: string;
    rateBasisPoints: number;
    active: boolean;
  }) => Promise<void>;
}) {
```

Prefill on open (lines 526-533):

old:
```tsx
  useEffect(() => {
    if (open) {
      setName("");
      setRate("");
      setActive(true);
      setError(null);
    }
  }, [open]);
```
new:
```tsx
  useEffect(() => {
    if (open) {
      setName(taxRate?.name ?? "");
      setRate(taxRate ? String(taxRate.rateBasisPoints / 100).replace(".", ",") : "");
      setActive(taxRate?.active ?? true);
      setError(null);
    }
  }, [open, taxRate]);
```
(`rateBasisPoints / 100` inverts the submit math `Math.round(normalizedRate * 100)`; `2000 → "20"`, `1000 → "10"`. The percent input already parses both comma and dot via `rate.replace(",", ".")`.)

Submit with the rate's id (lines 550-555):

old:
```tsx
      await onSave({
        id: null,
        name: name.trim(),
        rateBasisPoints: Math.round(normalizedRate * 100),
        active,
      });
```
new:
```tsx
      await onSave({
        id: taxRate?.id ?? null,
        name: name.trim(),
        rateBasisPoints: Math.round(normalizedRate * 100),
        active,
      });
```

- [ ] **Step 9: Run it, expect PASS** — `bunx vitest run src/app/settings/SettingsScreen.test.tsx`
  Expected: `Test Files  1 passed (1)` / `Tests  3 passed (3)`.

- [ ] **Step 10: Add backend regression tests (lock the update/deactivate + not_found contract).** Note: `save_tax_rate` already implements this, so these tests are expected to PASS on first run — they guard against regression and document "deactivate, not delete." Append inside `mod tests` in `src-tauri/src/commands/settings.rs`, immediately after `receipt_numbering_round_trip_persists_no_reset_policy` (after line 506):

```rust
    #[test]
    fn tax_rate_deactivate_keeps_row_in_list() {
        with_state("tax_rate_deactivate_keeps_row_in_list", |state| {
            let created = save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: None,
                    name: "PDV 20".to_string(),
                    rate_basis_points: 2000,
                    active: true,
                },
            )
            .expect("tax rate should be created");

            save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: Some(created.id),
                    name: created.name.clone(),
                    rate_basis_points: created.rate_basis_points,
                    active: false,
                },
            )
            .expect("tax rate should deactivate");

            let rates = list_tax_rates(state).expect("tax rates should list");
            let stored = rates
                .iter()
                .find(|rate| rate.id == created.id)
                .expect("deactivated rate should still be present");

            assert!(!stored.active);
        });
    }

    #[test]
    fn tax_rate_update_unknown_id_returns_not_found() {
        with_state("tax_rate_update_unknown_id_returns_not_found", |state| {
            let error = save_tax_rate(
                state,
                SaveTaxRateRequest {
                    id: Some(9_999),
                    name: "PDV 20".to_string(),
                    rate_basis_points: 2000,
                    active: false,
                },
            )
            .expect_err("updating a missing tax rate should fail");

            let command_error: crate::app_error::CommandError = error.into();

            assert_eq!(command_error.code, "not_found");
        });
    }
```
(Uses the existing `with_state` helper at settings.rs:387 and the `CommandError`-conversion assertion convention. The pre-existing `tax_rate_create_and_update_persists_active_state` at settings.rs:448-482 already covers the create→update happy path, so it is not duplicated.)

- [ ] **Step 11: Run backend tests, expect PASS** — `cd src-tauri && cargo test --locked tax_rate -- --test-threads=1`
  Expected: `test commands::settings::tests::tax_rate_deactivate_keeps_row_in_list ... ok`, `test commands::settings::tests::tax_rate_update_unknown_id_returns_not_found ... ok`, `test commands::settings::tests::tax_rate_create_and_update_persists_active_state ... ok`.

- [ ] **Step 12: Run full verification gates** —
  - `bun run test` → all suites pass.
  - `bun run build` → succeeds (no TS errors; `taxRate` prop typed).
  - `cd src-tauri && cargo test -- --test-threads=1` → all pass.
  - `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings` → no warnings.
  - `cd src-tauri && cargo fmt --check` → clean.

- [ ] **Final step: Commit** —
```
git add src/app/settings/SettingsScreen.tsx \
        src/app/settings/SettingsScreen.test.tsx \
        src-tauri/src/commands/settings.rs
git commit -m "feat(settings): VAT rate edit and deactivate workflow

Wire TaxRateDialog to edit existing rates (thread the rate id) and add
per-row Uredi/Deaktiviraj actions so VAT rates are deactivated, never
deleted. Add frontend edit/deactivate/validation tests and backend
deactivate + not_found regression tests.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: B2 (rec #6) — Receipts loading/empty/error states + reason text

**Files:**
- Test (create): `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.test.tsx`
- Modify: `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.tsx` — imports (after line 53), state block (after line 131, the `returnError` state), `runSearch` (lines 133–136), initial `useEffect` (lines 138–150), `showDetail` (lines 152–154), the receipt-list `<Table>…</Table>` (lines 381–426), the detail `<section>` (lines 429–445), and `ReceiptDetailPanel` (insert before the "Povezani dokumenti" block at line 628)
- No backend / `types.ts` / `ports.ts` / `receipts.rs` changes: `ReceiptDetail.voidReason` / `ReceiptDetail.returnReason` already exist (`/Users/adnan/Projects/vantumpos/src/services/types.ts:456-457`) and the Rust `load_receipt_detail` already populates them (`receipts.rs:643-644`). This task only surfaces them in the UI and hardens the read paths. The `userId: 1` hard-codes (ReceiptsScreen.tsx:173, 237) are owned by Task A1 — do NOT touch them here.

**Interfaces:**
- Consumes: `ReceiptsService` from `@/services/ports` (`searchReceipts(query): Promise<ReceiptSearchResult>`, `getReceipt(id): Promise<ReceiptDetail | null>`, `voidReceipt`, `returnItems`); existing UI primitives `Alert/AlertTitle/AlertDescription` (`@/components/ui/alert`, sets `role="alert"`), `Empty/EmptyHeader/EmptyMedia/EmptyTitle/EmptyDescription` (`@/components/ui/empty`), `Spinner` (`@/components/ui/spinner`); error shape `CommandErrorShape` (already imported, `types.ts:9`).
- Produces: no new exported symbols. New internal component state `listStatus: "loading" | "ready" | "error"`, `listError?: string`, `detailError?: string` inside `ReceiptsScreen`. Rendered Serbian strings later tests/callers key on: `"Ucitavanje racuna..."`, `"Racuni nisu ucitani"`, `"Nema racuna za izabrane filtere"`, `"Detalji nisu ucitani"`, `"Razlog storniranja"`, `"Razlog povrata"`.

---

- [ ] **Step 1: Write the failing test** — create `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReceiptsScreen } from "./ReceiptsScreen";
import type { ReceiptsService } from "@/services/ports";
import type {
  ReceiptDetail,
  ReceiptSearchResult,
  ReceiptSummary,
} from "@/services/types";

const summary: ReceiptSummary = {
  id: 1,
  receiptNumber: "R-2026-0001",
  createdAt: "2026-06-18T10:00:00Z",
  cashierName: "Mina Kasir",
  shiftId: 1,
  status: "voided",
  fiscalStatus: "not_fiscalized",
  documentType: "sale",
  paymentMethods: ["cash"],
  totalMinor: 100000,
  linkedDocumentCount: 1,
};

const detail: ReceiptDetail = {
  ...summary,
  subtotalMinor: 100000,
  discountMinor: 0,
  taxMinor: 16667,
  voidReason: "Pogresna stavka",
  returnReason: null,
  items: [
    {
      id: 1,
      productId: 2,
      productName: "Kafa 200 g",
      productSku: "KAFA-200",
      productBarcode: "8600000000027",
      quantityMilli: 2000,
      unitPriceMinor: 50000,
      discountMinor: 0,
      taxRateBasisPoints: 2000,
      taxMinor: 16667,
      totalMinor: 100000,
      returnedQuantityMilli: 0,
    },
  ],
  payments: [
    { id: 1, paymentMethod: "cash", amountMinor: 100000, createdAt: "2026-06-18T10:00:00Z" },
  ],
  linkedDocuments: [],
  canVoid: false,
  canReturn: false,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function buildReceiptsService(
  overrides: Partial<ReceiptsService> = {},
): ReceiptsService {
  return {
    searchReceipts: vi.fn(async () => ({ receipts: [summary], total: 1 })),
    getReceipt: vi.fn(async () => detail),
    voidReceipt: vi.fn(async () => detail),
    returnItems: vi.fn(async () => detail),
    ...overrides,
  };
}

describe("ReceiptsScreen", () => {
  it("shows a loading indicator while receipts load", async () => {
    const pending = deferred<ReceiptSearchResult>();
    render(
      <ReceiptsScreen
        receipts={buildReceiptsService({ searchReceipts: () => pending.promise })}
      />,
    );

    expect(await screen.findByText("Ucitavanje racuna...")).toBeInTheDocument();

    pending.resolve({ receipts: [summary], total: 1 });
    expect(await screen.findByText("R-2026-0001")).toBeInTheDocument();
  });

  it("shows an empty state when no receipts match", async () => {
    render(
      <ReceiptsScreen
        receipts={buildReceiptsService({
          searchReceipts: async () => ({ receipts: [], total: 0 }),
        })}
      />,
    );

    expect(
      await screen.findByText("Nema racuna za izabrane filtere"),
    ).toBeInTheDocument();
  });

  it("surfaces an error when the receipt list fails to load", async () => {
    const searchReceipts = vi
      .fn()
      .mockRejectedValue({ code: "database_error", message: "Baza nije dostupna." });
    render(<ReceiptsScreen receipts={buildReceiptsService({ searchReceipts })} />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Baza nije dostupna.");
  });

  it("surfaces an error when opening receipt detail fails", async () => {
    const user = userEvent.setup();
    const getReceipt = vi
      .fn()
      .mockRejectedValue({ code: "database_error", message: "Detalji nisu dostupni." });
    render(<ReceiptsScreen receipts={buildReceiptsService({ getReceipt })} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );

    expect(await screen.findByText("Detalji nisu dostupni.")).toBeInTheDocument();
  });

  it("renders the persisted void reason in the detail panel", async () => {
    const user = userEvent.setup();
    render(<ReceiptsScreen receipts={buildReceiptsService()} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );

    expect(await screen.findByText("Razlog storniranja")).toBeInTheDocument();
    expect(screen.getByText("Pogresna stavka")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it, expect FAIL** — `bunx vitest run src/app/ReceiptsScreen.test.tsx`
  Expected: the suite fails on the first test with `TestingLibraryElementError: Unable to find an element with the text: Ucitavanje racuna...` (the component renders the table immediately, with no loading/empty/error/reason markup yet).

- [ ] **Step 3a: Add the imports.** In `/Users/adnan/Projects/vantumpos/src/app/ReceiptsScreen.tsx`, after the `formatQuantity`/`parseQuantityInput` import (line 9) add the three primitive imports (alphabetical with the existing `@/components/ui/*` block):

```tsx
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
```
insert immediately after line 9 (`import { formatQuantity, parseQuantityInput } from "@/app/format";`), then add after the `AlertDialog` import group (line 19):

```tsx
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
```
and after the `Separator` import (line 35):

```tsx
import { Spinner } from "@/components/ui/spinner";
```

- [ ] **Step 3b: Add the three state hooks.** After line 131 (the `returnError` state), insert:

```tsx
  const [listStatus, setListStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [listError, setListError] = useState<string | undefined>();
  const [detailError, setDetailError] = useState<string | undefined>();
```

- [ ] **Step 3c: Harden `runSearch`.** Replace lines 133–136:

```tsx
  async function runSearch(query: ReceiptSearchQuery = {}) {
    const result = await receipts.searchReceipts(query);
    setRows(result.receipts);
  }
```
with:
```tsx
  async function runSearch(query: ReceiptSearchQuery = {}) {
    setListStatus("loading");
    setListError(undefined);

    try {
      const result = await receipts.searchReceipts(query);
      setRows(result.receipts);
      setListStatus("ready");
    } catch (error) {
      const commandError = error as CommandErrorShape;
      setListError(commandError.message ?? "Ucitavanje racuna nije uspelo.");
      setListStatus("error");
    }
  }
```

- [ ] **Step 3d: Harden the initial-load effect.** Replace lines 138–150:

```tsx
  useEffect(() => {
    let cancelled = false;

    receipts.searchReceipts({}).then((result) => {
      if (!cancelled) {
        setRows(result.receipts);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [receipts]);
```
with:
```tsx
  useEffect(() => {
    let cancelled = false;

    setListStatus("loading");
    setListError(undefined);

    receipts
      .searchReceipts({})
      .then((result) => {
        if (!cancelled) {
          setRows(result.receipts);
          setListStatus("ready");
        }
      })
      .catch((error) => {
        if (!cancelled) {
          const commandError = error as CommandErrorShape;
          setListError(commandError.message ?? "Ucitavanje racuna nije uspelo.");
          setListStatus("error");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [receipts]);
```

- [ ] **Step 3e: Harden `showDetail`.** Replace lines 152–154:

```tsx
  async function showDetail(id: number) {
    setSelected(await receipts.getReceipt(id));
  }
```
with:
```tsx
  async function showDetail(id: number) {
    setDetailError(undefined);

    try {
      setSelected(await receipts.getReceipt(id));
    } catch (error) {
      const commandError = error as CommandErrorShape;
      setDetailError(commandError.message ?? "Ucitavanje detalja nije uspelo.");
    }
  }
```

- [ ] **Step 3f: Add loading/empty/error states around the list table.** Replace the entire `<Table>…</Table>` block (lines 381–426) with the gated version:

```tsx
        {listStatus === "loading" ? (
          <div className="flex items-center gap-2 rounded-md border border-border p-4 text-sm text-muted-foreground">
            <Spinner aria-hidden="true" />
            Ucitavanje racuna...
          </div>
        ) : listStatus === "error" ? (
          <Alert variant="destructive">
            <AlertTitle>Racuni nisu ucitani</AlertTitle>
            <AlertDescription>{listError}</AlertDescription>
          </Alert>
        ) : rows.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <FileTextIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema racuna za izabrane filtere</EmptyTitle>
              <EmptyDescription>
                Promenite filtere ili napravite novu prodaju.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Broj</TableHead>
                <TableHead>Datum</TableHead>
                <TableHead>Kasir</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Fiskalno</TableHead>
                <TableHead>Placanje</TableHead>
                <TableHead>Ukupno</TableHead>
                <TableHead>Akcije</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.id}>
                  <TableCell>{row.receiptNumber}</TableCell>
                  <TableCell>{formatDateTime(row.createdAt)}</TableCell>
                  <TableCell>{row.cashierName}</TableCell>
                  <TableCell>
                    <Badge variant={row.status === "completed" ? "secondary" : "outline"}>
                      {statusLabels[row.status]}
                    </Badge>
                  </TableCell>
                  <TableCell>{fiscalSummaryLabels[row.fiscalStatus]}</TableCell>
                  <TableCell>
                    {row.paymentMethods
                      .map((method) => paymentSummaryLabels[method])
                      .join(", ")}
                  </TableCell>
                  <TableCell>{formatRsd(row.totalMinor)}</TableCell>
                  <TableCell>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => showDetail(row.id)}
                    >
                      <FileTextIcon data-icon="inline-start" />
                      Detalji za {row.receiptNumber}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
```

- [ ] **Step 3g: Render the detail-read error.** In the detail `<section>` (lines 429–445), insert the alert as the first child, immediately before `{selected ? (`:

```tsx
      <section className="flex min-w-0 flex-col gap-4">
        {detailError ? (
          <Alert variant="destructive">
            <AlertTitle>Detalji nisu ucitani</AlertTitle>
            <AlertDescription>{detailError}</AlertDescription>
          </Alert>
        ) : null}
        {selected ? (
```
(leave the existing `<ReceiptDetailPanel …/>` / "Izaberite racun za detalje." branch unchanged below it).

- [ ] **Step 3h: Surface the persisted reason text in `ReceiptDetailPanel`.** Insert before the "Povezani dokumenti" block (the `<div className="flex flex-col gap-2">` whose `<h3>` is `Povezani dokumenti`, at line 628):

```tsx
      {detail.voidReason ? (
        <div className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">Razlog storniranja</h3>
          <p className="text-sm text-muted-foreground">{detail.voidReason}</p>
        </div>
      ) : null}

      {detail.returnReason ? (
        <div className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">Razlog povrata</h3>
          <p className="text-sm text-muted-foreground">{detail.returnReason}</p>
        </div>
      ) : null}

```

- [ ] **Step 4: Run it, expect PASS** — `bunx vitest run src/app/ReceiptsScreen.test.tsx`
  Expected: `Test Files  1 passed (1)` / `Tests  5 passed (5)` (loading, empty, list error, detail error, void-reason).

- [ ] **Step 5: Run the full gates** — `bun run test` (expect the existing suite plus the new 5 to pass) and `bun run build` (expect `tsc` + `vite build` to complete with no type errors — confirms the new `listStatus` union, the `Empty`/`Spinner`/`Alert` imports, and the `detail.voidReason`/`detail.returnReason` accesses type-check).

- [ ] **Final step: Commit**
```
git add src/app/ReceiptsScreen.tsx src/app/ReceiptsScreen.test.tsx
git commit -m "feat(receipts): add list loading/empty/error states and reason text

Wrap receipt search and detail reads in try/catch (no more unhandled
rejections), render loading/empty/error states for the receipt list,
surface the persisted void/return reason in the detail panel.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

I have all the ground truth confirmed. Here is the task section.

---

### Task 8: B3 (rec #7) — Reports shift/cashier filtering, role-gate, FE state tests

**Files:**
- Modify `src-tauri/src/commands/reports.rs` — extend `ReportDateQuery` (struct at lines 11-16), `ProductSalesQuery::date_query()` (lines 27-34), and add filters to `query_shift_turnover` (lines 357-406) + `query_cashier_turnover` (lines 408-439).
- Test `src-tauri/src/commands/reports.rs` — add two `#[test]` fns inside the existing `mod tests` (after line 1141, reusing `with_seeded_reports_database` at lines 823-839 and `seed_reports_data` at lines 841-1046); update the two existing `ReportDateQuery` literals at lines 1055-1058 and 1079-1082.
- Modify `src/services/types.ts` — `ReportDateQuery` interface (lines 486-489).
- Test `src/services/reports-adapter.test.ts` — add one `it` block (file is lines 1-78).
- Modify `src/app/reports/ReportsScreen.tsx` — props (lines 96-101), imports (lines 16, 58-69), `useEffect` (lines 153-155), `applyFilters` (lines 157-160), add role gate + `validateDateRange` helper.
- Test `src/app/reports/ReportsScreen.test.tsx` — update `renderReports` (lines 109-120), add user fixtures + 4 `it` blocks.
- Modify `src/app/AppShell.tsx` — `renderModule` reports branch (lines 364-366).
- Modify `src/app/navigation.ts` — `reports` nav item (lines 40-44).

**Interfaces:**
- Consumes (from Task 2 / A2): the `NavigationItem.adminOnly?: boolean` field added to the `NavigationItem` interface in `src/app/navigation.ts`, plus AppShell's nav-render filter at `AppShell.tsx:220` that hides `adminOnly` items for `session.user.role !== "admin"`. (Task 2 DEFINES this for the `settings` item; Task 8 only sets `adminOnly: true` on the `reports` item.)
- Consumes (existing): `UserAccount` (`src/services/types.ts:95-104`, has `role: UserRole`), `Alert`/`AlertTitle`/`AlertDescription` from `@/components/ui/alert`, `with_seeded_reports_database` + `seed_reports_data` test helpers in `reports.rs`.
- Produces:
  - Rust: `ReportDateQuery { from: String, to: String, shift_id: Option<i64>, cashier_id: Option<i64> }` — new optional shift/cashier filters applied by `query_shift_turnover` and `query_cashier_turnover`.
  - TS: `ReportDateQuery` gains optional `shiftId?: number | null; cashierId?: number | null;` (forwarded by the existing `local-adapter.ts` reports block, lines 144-147).
  - React: `ReportsScreen({ reports, currentUser, initialQuery })` — now requires `currentUser: UserAccount` and renders an admin-only gate.

---

- [ ] **Step 1: Write the failing Rust filter tests.** Append inside `mod tests` in `src-tauri/src/commands/reports.rs` (after the `query_low_stock...` test ends at line 1141):

```rust
    #[test]
    fn query_shift_turnover_filters_by_shift_id() {
        with_seeded_reports_database(
            "query_shift_turnover_filters_by_shift_id",
            |connection| {
                let matched = super::query_shift_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: Some(1),
                        cashier_id: None,
                    },
                )
                .expect("shift turnover should query");
                assert_eq!(matched.rows.len(), 1);
                assert_eq!(matched.rows[0].shift_id, 1);

                let unmatched = super::query_shift_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: Some(999),
                        cashier_id: None,
                    },
                )
                .expect("shift turnover should query");
                assert!(unmatched.rows.is_empty());
            },
        );
    }

    #[test]
    fn query_cashier_turnover_filters_by_cashier_id() {
        with_seeded_reports_database(
            "query_cashier_turnover_filters_by_cashier_id",
            |connection| {
                let matched = super::query_cashier_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: None,
                        cashier_id: Some(2),
                    },
                )
                .expect("cashier turnover should query");
                assert_eq!(matched.rows.len(), 1);
                assert_eq!(matched.rows[0].cashier_id, 2);

                let unmatched = super::query_cashier_turnover(
                    connection,
                    &super::ReportDateQuery {
                        from: "2026-06-17".to_string(),
                        to: "2026-06-17".to_string(),
                        shift_id: None,
                        cashier_id: Some(999),
                    },
                )
                .expect("cashier turnover should query");
                assert!(unmatched.rows.is_empty());
            },
        );
    }
```

(The seed at `reports.rs:841-1046` already inserts shift id 1 / cashier id 2 "Mira Kasir" with three sales; cashier id 1 is the `Db::new` bootstrap admin with no sales.)

- [ ] **Step 2: Run it, expect FAIL.** `cd src-tauri && cargo test filters_by -- --test-threads=1` → fails to compile: `error[E0560]: struct \`ReportDateQuery\` has no field named \`shift_id\`` (the struct still only has `from`/`to`).

- [ ] **Step 3: Implement the backend filters.** In `src-tauri/src/commands/reports.rs`:

Extend the struct (lines 11-16):
```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDateQuery {
    pub from: String,
    pub to: String,
    pub shift_id: Option<i64>,
    pub cashier_id: Option<i64>,
}
```
(serde maps these to `shiftId`/`cashierId`; missing `Option` fields deserialize to `None`, so existing FE callers that send only `{from,to}` still work.)

Update `date_query()` (lines 28-33) so the literal compiles:
```rust
    fn date_query(&self) -> ReportDateQuery {
        ReportDateQuery {
            from: self.from.clone(),
            to: self.to.clone(),
            shift_id: None,
            cashier_id: None,
        }
    }
```

In `query_shift_turnover`, change the `WHERE` (line 384) and the `query_map` params (line 391):
```rust
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
  AND (?3 IS NULL OR sh.id = ?3)
  AND (?4 IS NULL OR sh.user_id = ?4)
GROUP BY sh.id, sh.opened_at, sh.closed_at, u.display_name
```
```rust
        .query_map(params![query.from, query.to, query.shift_id, query.cashier_id], |row| {
```

In `query_cashier_turnover`, change the `WHERE` (line 421) and params (line 428):
```rust
WHERE substr(s.created_at, 1, 10) BETWEEN ?1 AND ?2
  AND (?3 IS NULL OR s.shift_id = ?3)
  AND (?4 IS NULL OR u.id = ?4)
GROUP BY u.id, u.display_name
```
```rust
        .query_map(params![query.from, query.to, query.shift_id, query.cashier_id], |row| {
```

Update the two pre-existing test literals so they still compile — at lines 1055-1058 (`query_daily_turnover_groups...`) and 1079-1082 (`query_payment_methods_handles...`), append `shift_id: None, cashier_id: None,` to each `super::ReportDateQuery { from: ..., to: ... }`.

- [ ] **Step 4: Run it, expect PASS.** `cd src-tauri && cargo test reports -- --test-threads=1` → all reports tests pass (incl. the two new `filters_by` tests). Then `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings` and `cd src-tauri && cargo fmt --check` clean.

- [ ] **Step 5: Write the failing FE adapter test.** Add to `src/services/reports-adapter.test.ts` inside the `describe` (after line 76):

```ts
  it("forwards shift and cashier filters in the query envelope", async () => {
    const invoke = vi.fn().mockResolvedValue({ rows: [] });
    const services = createLocalServices(invoke);

    await services.reports.getShiftTurnover({
      from: "2026-06-17",
      to: "2026-06-17",
      shiftId: 1,
      cashierId: 2,
    });

    expect(invoke).toHaveBeenCalledWith("reports_shift_turnover", {
      query: { from: "2026-06-17", to: "2026-06-17", shiftId: 1, cashierId: 2 },
    });
  });
```

- [ ] **Step 6: Run it, expect FAIL.** `bun run test reports-adapter` → fails: `error TS2353: Object literal may only specify known properties, and 'shiftId' does not exist in type 'ReportDateQuery'`.

- [ ] **Step 7: Implement the TS contract.** In `src/services/types.ts` (lines 486-489):
```ts
export interface ReportDateQuery {
  from: string;
  to: string;
  shiftId?: number | null;
  cashierId?: number | null;
}
```
No adapter code change needed — `local-adapter.ts:144-147` already forwards the whole `query` object as `{ query }`.

- [ ] **Step 8: Run it, expect PASS.** `bun run test reports-adapter` → the new forwarding test and the existing mapping test both pass.

- [ ] **Step 9: Write the failing FE screen tests.** In `src/app/reports/ReportsScreen.test.tsx`, add the type import and fixtures after line 8, replace `renderReports`, add `buildEmptyReportsService`, and add 4 `it` blocks.

Add import (after line 6):
```ts
import type { UserAccount } from "@/services/types";
```

Add fixtures + empty-service builder (after `buildReportsService` ends at line 107):
```tsx
const adminUser: UserAccount = {
  id: 1,
  username: "admin",
  displayName: "Administrator",
  role: "admin",
  active: true,
  createdAt: "2026-06-17T07:00:00Z",
  updatedAt: "2026-06-17T07:00:00Z",
  lastLoginAt: null,
};

const cashierUser: UserAccount = {
  ...adminUser,
  id: 2,
  username: "mira",
  displayName: "Mira Kasir",
  role: "cashier",
};

function buildEmptyReportsService(): ReportsService {
  const service = buildReportsService();
  service.getDailyTurnover = vi.fn().mockResolvedValue({
    summary: {
      totalMinor: 0,
      cashMinor: 0,
      cardMinor: 0,
      receiptCount: 0,
      averageReceiptMinor: 0,
    },
    rows: [],
  });
  service.getShiftTurnover = vi.fn().mockResolvedValue({ rows: [] });
  service.getCashierTurnover = vi.fn().mockResolvedValue({ rows: [] });
  service.getPaymentMethodTurnover = vi.fn().mockResolvedValue({ rows: [] });
  service.getProductSales = vi.fn().mockResolvedValue({ rows: [] });
  service.getCategorySales = vi.fn().mockResolvedValue({ rows: [] });
  service.getLowStock = vi.fn().mockResolvedValue({ rows: [] });
  return service;
}
```

Replace `renderReports` (lines 109-120) to thread `currentUser`:
```tsx
function renderReports(
  service = buildReportsService(),
  currentUser: UserAccount = adminUser,
) {
  render(
    <>
      <ReportsScreen
        reports={service}
        currentUser={currentUser}
        initialQuery={{ from: "2026-06-17", to: "2026-06-17" }}
      />
      <Toaster />
    </>,
  );
  return service;
}
```

Add inside `describe("ReportsScreen", ...)` (after the existing export test at line 185):
```tsx
  it("blocks non-admin operators with a role notice and skips loading", () => {
    const reports = renderReports(buildReportsService(), cashierUser);

    expect(
      screen.getByText("Samo administrator moze da vidi izvestaje."),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Dnevni promet" }),
    ).not.toBeInTheDocument();
    expect(reports.getDailyTurnover).not.toHaveBeenCalled();
  });

  it("rejects an inverted date range without calling the service", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.clear(screen.getByLabelText("Od datuma"));
    await user.type(screen.getByLabelText("Od datuma"), "2026-06-20");
    await user.click(screen.getByRole("button", { name: "Primeni filtere" }));

    expect(
      screen.getByText("Pocetni datum ne sme biti posle krajnjeg datuma."),
    ).toBeInTheDocument();
    expect(reports.getDailyTurnover).toHaveBeenCalledTimes(1);
  });

  it("renders empty states when reports return no rows", async () => {
    const user = userEvent.setup();
    renderReports(buildEmptyReportsService());

    expect(await screen.findByText("Nema prometa")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Lager" }));
    expect(
      await screen.findByText("Nema artikala ispod minimuma"),
    ).toBeInTheDocument();
  });

  it("surfaces a backend failure in an alert", async () => {
    const reports = buildReportsService();
    reports.getDailyTurnover = vi.fn().mockRejectedValue({
      code: "database_error",
      message: "Izvestaj nije dostupan.",
    });
    renderReports(reports);

    expect(
      await screen.findByText("Izvestaj nije dostupan."),
    ).toBeInTheDocument();
  });
```

- [ ] **Step 10: Run it, expect FAIL.** `bun run test ReportsScreen` → the role-gate test fails (`Unable to find an element with the text: Samo administrator moze da vidi izvestaje.` — gate not implemented, and `getDailyTurnover` was called) and the date-validation test fails (`expected "getDailyTurnover" to be called 1 times, but it was called 2 times` — invalid range still loads). The empty/failure tests already pass against existing render logic.

- [ ] **Step 11: Implement the gate, effect guard, and validation.** In `src/app/reports/ReportsScreen.tsx`:

Imports — line 16 add `AlertTitle`:
```tsx
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
```
Types import (lines 58-69) — add `UserAccount`:
```tsx
import type { ReportsService } from "@/services/ports";
import type {
  CashierTurnoverReport,
  CategorySalesReport,
  DailyTurnoverReport,
  ExportReportType,
  LowStockReport,
  PaymentMethodReport,
  ProductSalesQuery,
  ProductSalesReport,
  ReportDateQuery,
  ShiftTurnoverReport,
  UserAccount,
} from "@/services/types";
```

Props (lines 96-101):
```tsx
interface ReportsScreenProps {
  reports: ReportsService;
  currentUser: UserAccount;
  initialQuery?: ReportDateQuery;
}

export function ReportsScreen({
  reports,
  currentUser,
  initialQuery,
}: ReportsScreenProps) {
```

Guard the mount load — `useEffect` (lines 153-155):
```tsx
  useEffect(() => {
    if (currentUser.role !== "admin") {
      return;
    }
    void loadReports(defaultQuery);
  }, [currentUser.role, defaultQuery, loadReports]);
```

Validate before applying — `applyFilters` (lines 157-160):
```tsx
  function applyFilters(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const validationError = validateDateRange(filters);
    if (validationError) {
      setErrorMessage(validationError);
      return;
    }
    void loadReports(filters);
  }
```

Add the role gate immediately before the main `return (` (after `exportCsv`, around line 177 — placed after all hooks to respect the Rules of Hooks, mirroring `UsersScreen` at `AppShell.tsx:867-876`):
```tsx
  if (currentUser.role !== "admin") {
    return (
      <Alert>
        <AlertTitle>Izvestaji</AlertTitle>
        <AlertDescription>
          Samo administrator moze da vidi izvestaje.
        </AlertDescription>
      </Alert>
    );
  }
```

Add the validation helper next to `todayQuery` (after line 673):
```tsx
function validateDateRange(query: ReportDateQuery): string | null {
  if (!query.from || !query.to) {
    return "Izaberite pocetni i krajnji datum.";
  }
  if (query.from > query.to) {
    return "Pocetni datum ne sme biti posle krajnjeg datuma.";
  }
  return null;
}
```

Thread the session user in `src/app/AppShell.tsx` `renderModule` (lines 364-366):
```tsx
  if (activeId === "reports") {
    return <ReportsScreen reports={services.reports} currentUser={session.user} />;
  }
```

Hide the nav item for cashiers in `src/app/navigation.ts` (lines 40-44), consuming Task 2's `adminOnly` field:
```tsx
  {
    id: "reports",
    label: "Izvestaji",
    icon: BarChart3Icon,
    adminOnly: true,
  },
```

- [ ] **Step 12: Run it, expect PASS.** `bun run test ReportsScreen` → all 7 tests pass (3 existing + 4 new). Then `bun run test` (full suite) and `bun run build` (tsc + vite) green.

- [ ] **Final step: Run all gates and commit.**
  - `bun run test`
  - `bun run build`
  - `cd src-tauri && cargo test -- --test-threads=1`
  - `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cd src-tauri && cargo fmt --check`
  - Then:
```bash
git add src-tauri/src/commands/reports.rs src/services/types.ts \
  src/services/reports-adapter.test.ts src/app/reports/ReportsScreen.tsx \
  src/app/reports/ReportsScreen.test.tsx src/app/AppShell.tsx src/app/navigation.ts
git commit -m "feat(reports): add shift/cashier query filters, admin role-gate, and FE state tests

Extend ReportDateQuery with optional shiftId/cashierId and apply them in
query_shift_turnover and query_cashier_turnover. Role-gate ReportsScreen via
currentUser (admin-only), hide the Izvestaji nav item for cashiers, and add
date-range validation plus empty/failure-state tests.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

I have all the ground truth confirmed. Here is the task section.

### Task 9: B4 (rec #8) — Remove dead catalog screen, deep-link product ledger

**Files:**
- Delete: `/Users/adnan/Projects/vantumpos/src/app/ProductCatalogScreen.tsx` (orphaned; only self-references at lines 38, 55 — verified no external importer)
- Modify: `/Users/adnan/Projects/vantumpos/src/app/catalog/CatalogModule.tsx` (prop type line 93; `ProductTable` prop type line 908; "Lager" button `onClick` line 983)
- Modify: `/Users/adnan/Projects/vantumpos/src/app/inventory/InventoryScreen.tsx` (props interface lines 77-79; component signature line 98; `openLedger` lines 141-152; new deep-link effect after line 152; `StockTable` prop type line 325; Ledger button `onClick` line 396)
- Modify: `/Users/adnan/Projects/vantumpos/src/app/AppShell.tsx` (new state near line 120; `renderModule` signature lines 307-318; products branch lines 346-353; inventory branch lines 368-370; `renderModule` call lines 291-298)
- Test: `/Users/adnan/Projects/vantumpos/src/App.test.tsx` (append two `it(...)` cases inside the existing `describe("AppShell", …)` block)

**Interfaces:**
- Consumes (existing, unchanged): `InventoryService.getProductLedger(productId: number): Promise<ProductLedger>` (`src/services/ports.ts`; mock at `mock-adapter.ts:579-586`); `CatalogService.listProducts`; mock seed product id 1 = "Mleko 1 l" / sku "MLEKO-1L" (`mock-adapter.ts:38-57`). This task does NOT consume Task 2's role helper — the catalog→ledger deep link is role-agnostic.
- Produces:
  - `CatalogModuleProps.onOpenInventory: (productId: number) => void` (was `() => void`) — caller passes `product.id`.
  - `InventoryScreenProps.initialLedgerProductId?: number | null` and `InventoryScreenProps.onLedgerOpened?: () => void` — when `initialLedgerProductId` is a number, the ledger Sheet auto-opens for that product and `onLedgerOpened` fires once.
  - `renderModule` new params: `inventoryLedgerProductId: number | null`, `onOpenProductLedger: (productId: number) => void`, `onInventoryLedgerOpened: () => void`.

---

- [ ] **Step 1: Write the failing tests** — append to `/Users/adnan/Projects/vantumpos/src/App.test.tsx` inside `describe("AppShell", () => { … })`. The imports `render, screen, waitFor, within` and `userEvent`, `createMockServices` already exist at the top of the file (per repo convention).

```tsx
  it("deep-links the catalog Lager action to the product ledger", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    expect(
      await screen.findByRole("cell", { name: "MLEKO-1L" }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Lager za Mleko 1 l" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Stanje lagera" }),
    ).toBeInTheDocument();

    const ledger = await screen.findByRole("dialog", {
      name: "Kartica artikla",
    });
    expect(within(ledger).getByText("Mleko 1 l")).toBeInTheDocument();
    expect(within(ledger).getByText(/MLEKO-1L/)).toBeInTheDocument();
  });

  it("surfaces an error when the deep-linked ledger fails to load", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    services.inventory.getProductLedger = () =>
      Promise.reject({ code: "not_found", message: "Artikal nije pronadjen." });

    render(<AppShell services={services} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(
      await screen.findByRole("button", { name: "Lager za Mleko 1 l" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Stanje lagera" }),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("Artikal nije pronadjen."),
    ).toBeInTheDocument();
  });
```

Rationale: the happy path proves clicking "Lager za Mleko 1 l" both navigates to the Lager screen AND auto-opens that product's ledger Sheet (`within(ledger)` scoping disambiguates the product name/SKU that also appear in the StockTable). The failure path proves a rejected `getProductLedger` surfaces the Serbian toast via the shell's `<Toaster />` (`AppShell.tsx:301`) without crashing navigation.

- [ ] **Step 2: Run them, expect FAIL** — currently the "Lager" button calls `onOpenInventory()` with no id and `onOpenInventory={() => onNavigate("inventory")}` only switches tabs; no ledger opens and `getProductLedger` is never called.

```
bunx vitest run src/App.test.tsx -t "deep-link"
```
Expected: 1 failed — `TestingLibraryElementError: Unable to find an accessible element with the role "dialog" and name "Kartica artikla"`.

```
bunx vitest run src/App.test.tsx -t "deep-linked ledger fails"
```
Expected: 1 failed — `Unable to find an element with the text: Artikal nije pronadjen.` (because `getProductLedger` is never invoked).

- [ ] **Step 3: Delete the orphaned screen.**

```
git rm src/app/ProductCatalogScreen.tsx
```
Then re-confirm nothing imports it (must print nothing):
```
grep -rn "ProductCatalogScreen" /Users/adnan/Projects/vantumpos/src --include=*.tsx --include=*.ts
```
Expected output: empty (the only prior references were inside the deleted file).

- [ ] **Step 4: Thread the product id through `CatalogModule.tsx`.**

Edit the `CatalogModuleProps` prop type (line 93):
```tsx
  onOpenInventory: (productId: number) => void;
```
Edit the `ProductTable` prop type (line 908) to match:
```tsx
  onOpenInventory: (productId: number) => void;
```
Edit the "Lager" row button (line 983) to pass the row's product id:
```tsx
                    onClick={() => onOpenInventory(product.id)}
```
(The destructure at line 158, the pass-through at line 728 `onOpenInventory={onOpenInventory}`, and the `aria-label={`Lager za ${product.name}`}` at line 982 stay unchanged.)

- [ ] **Step 5: Make `InventoryScreen.tsx` accept and consume the deep-link prop.**

Extend the props interface (lines 77-79):
```tsx
interface InventoryScreenProps {
  services: PosServices;
  initialLedgerProductId?: number | null;
  onLedgerOpened?: () => void;
}
```
Update the component signature (line 98):
```tsx
export function InventoryScreen({
  services,
  initialLedgerProductId = null,
  onLedgerOpened,
}: InventoryScreenProps) {
```
Refactor `openLedger` to take a product id instead of a `StockListItem` (replace lines 141-152):
```tsx
  async function openLedger(productId: number) {
    setLedgerOpen(true);
    setLedgerLoading(true);

    try {
      setLedger(await services.inventory.getProductLedger(productId));
    } catch (unknownError) {
      toast.error(getCommandMessage(unknownError));
    } finally {
      setLedgerLoading(false);
    }
  }

  useEffect(() => {
    if (initialLedgerProductId == null) {
      return;
    }

    void openLedger(initialLedgerProductId);
    onLedgerOpened?.();
  }, [initialLedgerProductId]);
```
(`openLedger` is a hoisted function declaration, so referencing it from the effect above its lexical position is valid. The effect intentionally depends only on `initialLedgerProductId`, mirroring the existing `useEffect(() => { void refreshStock(query); }, [query]);` at lines 137-139 — `bun run build` is `tsc && vite build`, no eslint exhaustive-deps gate.)

Update the `StockTable` prop type (line 325):
```tsx
  onLedger: (productId: number) => void;
```
Update the Ledger row button (line 396) to pass the id:
```tsx
                  onClick={() => onLedger(item.productId)}
```
(The `onLedger={openLedger}` pass at line 270 needs no change — `openLedger` now matches `(productId: number) => void`.)

- [ ] **Step 6: Hold the focused product id in `AppShell.tsx` and wire `renderModule`.**

Add state next to `activeId` (after line 120):
```tsx
  const [inventoryLedgerProductId, setInventoryLedgerProductId] = useState<
    number | null
  >(null);
```
Extend the `renderModule` parameter list and its type (lines 307-318):
```tsx
function renderModule({
  activeId,
  session,
  services,
  onNavigate,
  onSessionChange,
  inventoryLedgerProductId,
  onOpenProductLedger,
  onInventoryLedgerOpened,
}: {
  activeId: NavigationItemId;
  session: AppSession;
  services: PosServices;
  onNavigate: (id: NavigationItemId) => void;
  onSessionChange: (session: AppSession) => void;
  inventoryLedgerProductId: number | null;
  onOpenProductLedger: (productId: number) => void;
  onInventoryLedgerOpened: () => void;
}) {
```
Replace the products branch (lines 346-353):
```tsx
  if (activeId === "products") {
    return (
      <CatalogModule services={services} onOpenInventory={onOpenProductLedger} />
    );
  }
```
Replace the inventory branch (lines 368-370):
```tsx
  if (activeId === "inventory") {
    return (
      <InventoryScreen
        services={services}
        initialLedgerProductId={inventoryLedgerProductId}
        onLedgerOpened={onInventoryLedgerOpened}
      />
    );
  }
```
Update the `renderModule(...)` call (lines 291-298) to supply the new handlers:
```tsx
            {renderModule({
              activeId,
              session,
              services,
              onNavigate: setActiveId,
              onSessionChange: (nextSession) =>
                setSessionState({ status: "ready", session: nextSession }),
              inventoryLedgerProductId,
              onOpenProductLedger: (productId) => {
                setInventoryLedgerProductId(productId);
                setActiveId("inventory");
              },
              onInventoryLedgerOpened: () => setInventoryLedgerProductId(null),
            })}
```
Flow: the "Lager" button calls `onOpenInventory(product.id)` → `onOpenProductLedger` sets `inventoryLedgerProductId` and switches to `"inventory"` → `InventoryScreen` mounts with `initialLedgerProductId`, its effect opens the ledger and fires `onLedgerOpened`, which resets the id to `null` (so the effect's next run early-returns; the ledger opens exactly once and does not re-open on a later manual visit).

- [ ] **Step 7: Run the two tests, expect PASS.**

```
bunx vitest run src/App.test.tsx -t "deep-link"
```
Expected: `2 passed` (both the happy "deep-links the catalog Lager action…" and the "deep-linked ledger fails to load" cases match the `-t` substring).

- [ ] **Step 8: Run the full verification gates.**

```
bun run test
bun run build
```
Expected: `bun run test` — all suites pass (including the unchanged `src/services/local-adapter.test.ts` which still references `getProductLedger`); `bun run build` — `tsc` typechecks clean (the deleted `ProductCatalogScreen.tsx` has no importers) and `vite build` succeeds. No Rust changed in this task, but the repo-wide gates remain green:
```
cd /Users/adnan/Projects/vantumpos/src-tauri && cargo test -- --test-threads=1
cd /Users/adnan/Projects/vantumpos/src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings
cd /Users/adnan/Projects/vantumpos/src-tauri && cargo fmt --check
```

- [ ] **Final step: Commit.**

```
git add src/app/catalog/CatalogModule.tsx src/app/inventory/InventoryScreen.tsx src/app/AppShell.tsx src/App.test.tsx
git commit -m "feat: deep-link catalog Lager action to product ledger and drop dead screen

Remove orphaned ProductCatalogScreen.tsx and make the catalog product
\"Lager\" row action open that product's inventory ledger directly via a
threaded productId, instead of merely switching to the Lager tab.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
(The `git rm` from Step 3 already staged the deletion of `src/app/ProductCatalogScreen.tsx`, so it is included in this commit.)

---

### Task 10: B5 (rec #9) — Import job-detail drill-down + unknown-VAT/required-mapping tests

**Files:**
- Test (backend, modify — insert before the `mod tests` closing brace at `src-tauri/src/importer.rs:1697`, after the last test ending at `1696`): `/Users/adnan/Projects/vantumpos/src-tauri/src/importer.rs`
- Test (frontend, modify — add an `it(...)` inside the existing import `describe` block, after `src/App.test.tsx:702`): `/Users/adnan/Projects/vantumpos/src/App.test.tsx`
- Modify (mock drill-down rows): `/Users/adnan/Projects/vantumpos/src/services/mock-adapter.ts:716-727`
- Modify (UI drill-down): `/Users/adnan/Projects/vantumpos/src/app/import/ImportWizard.tsx` (imports `1-75`; `ImportHistory` call site `425`; `ImportHistory` component `505-555`)
- Modify (docs count): `/Users/adnan/Projects/vantumpos/docs/PROGRESS.md:572,574`

**Interfaces:**
- Consumes (all already exist — no new command/port needed):
  - Backend command `import_get_job(state, id: i64) -> Result<ImportJobDetail, CommandError>` (`src-tauri/src/commands/imports.rs:39-45`), delegating to `get_import_job(db: &Db, id: i64) -> Result<ImportJobDetail, ImportError>` (`src-tauri/src/importer.rs:462-505`).
  - Port `ImportService.getImportJob(id: number): Promise<ImportJobDetail | null>` (`src/services/ports.ts:137`); local adapter `getImportJob: (id) => invoke<ImportJobDetail | null>("import_get_job", { id })` (`src/services/local-adapter.ts:139`); mock `getImportJob(id)` (`src/services/mock-adapter.ts:716-727`).
  - Validation inner fn `validate_import(db: &Db, &ValidateImportRequest) -> Result<ImportValidationResult, ImportError>`; row error message constants `"PDV stopa nije pronadjena."` (`importer.rs:582`) and `"Mapirajte kolicinu i sifru ili barcode artikla."` (`importer.rs:655`).
  - Test helpers in the `importer.rs` test module: `mapping(&[(&str,&str)])` (`importer.rs:1418`), `with_test_database(&str, |db|)` (`importer.rs:1425`), `seed_tax_rate(db, name, basis_points)` (`importer.rs:1441`). Backend enums `ImportType`, `ImportRowStatus` are in scope via `use super::*;` (`importer.rs:1415`).
- Produces:
  - Backend tests `validate_products_flags_unknown_vat_rate_as_error`, `validate_products_accepts_known_vat_rate`, `validate_initial_stock_requires_quantity_and_sku_or_barcode_mapping` (raise importer.rs `#[test]` count from 7 → 10).
  - Frontend: a `ImportJobDetailDialog` (in `ImportWizard.tsx`) and a per-row trigger button labelled `Detalji importa ${job.fileName}` that opens it.

Note: the products required-mapping case is already covered by `validate_products_requires_sale_price_vat_and_sku_or_barcode_mapping` (`importer.rs:1506-1527`); this task adds the genuinely-missing unknown-VAT cases and the initial-stock required-mapping case.

---

- [ ] **Step 1 — Write backend tests (unknown-VAT failure + VAT-resolved happy + initial-stock required-mapping failure).** Insert into `src-tauri/src/importer.rs` immediately before the `mod tests` closing brace at line `1697` (i.e. after the `commit_initial_stock_writes_receive_movement_for_existing_sku` test that ends at `1696`):

```rust
    #[test]
    fn validate_products_flags_unknown_vat_rate_as_error() {
        with_test_database("validate_products_flags_unknown_vat_rate_as_error", |db| {
            // No tax rate seeded, so the 20% column cannot resolve to a tax_rates row.
            let request = ValidateImportRequest {
                import_type: ImportType::Products,
                file_name: "artikli.csv".to_string(),
                csv_text: "Naziv;Cena;PDV;Sifra\nHleb;120,00;20;SKU-1\n".to_string(),
                mapping: mapping(&[
                    ("name", "Naziv"),
                    ("sale_price", "Cena"),
                    ("vat_rate", "PDV"),
                    ("sku", "Sifra"),
                ]),
            };

            let result = validate_import(db, &request).expect("validation should run");

            assert_eq!(result.error_count, 1);
            assert_eq!(result.rows[0].row_number, 2);
            assert_eq!(result.rows[0].status, ImportRowStatus::Error);
            assert_eq!(result.rows[0].message, "PDV stopa nije pronadjena.");
        });
    }

    #[test]
    fn validate_products_accepts_known_vat_rate() {
        with_test_database("validate_products_accepts_known_vat_rate", |db| {
            seed_tax_rate(db, "PDV 20", 2000);
            let request = ValidateImportRequest {
                import_type: ImportType::Products,
                file_name: "artikli.csv".to_string(),
                csv_text: "Naziv;Cena;PDV;Sifra\nHleb;120,00;20;SKU-1\n".to_string(),
                mapping: mapping(&[
                    ("name", "Naziv"),
                    ("sale_price", "Cena"),
                    ("vat_rate", "PDV"),
                    ("sku", "Sifra"),
                ]),
            };

            let result = validate_import(db, &request).expect("validation should run");

            assert_eq!(result.error_count, 0);
            assert_eq!(result.rows[0].status, ImportRowStatus::Valid);
            assert_eq!(result.rows[0].action, ImportRowAction::Create);
        });
    }

    #[test]
    fn validate_initial_stock_requires_quantity_and_sku_or_barcode_mapping() {
        with_test_database(
            "validate_initial_stock_requires_quantity_and_sku_or_barcode_mapping",
            |db| {
                let request = ValidateImportRequest {
                    import_type: ImportType::InitialStock,
                    file_name: "stanje.csv".to_string(),
                    csv_text: "Sifra;Kolicina\nSKU-1;2\n".to_string(),
                    mapping: mapping(&[("sku", "Sifra")]),
                };

                let result = validate_import(db, &request).expect("validation should run");

                assert_eq!(result.error_count, 1);
                assert_eq!(
                    result.rows[0].message,
                    "Mapirajte kolicinu i sifru ili barcode artikla."
                );
            },
        );
    }
```

- [ ] **Step 2 — Run backend tests, expect PASS (characterization of existing validation that was previously untested).** These exercise behavior that already exists (`importer.rs:575-582` for unknown VAT, `importer.rs:653-657` for initial-stock mapping), so they go green on first run; they exist to close the spec-named coverage gap and guard against regression.
```
cd /Users/adnan/Projects/vantumpos/src-tauri && cargo test --lib importer -- --test-threads=1
```
Expected output includes:
```
test importer::tests::validate_products_flags_unknown_vat_rate_as_error ... ok
test importer::tests::validate_products_accepts_known_vat_rate ... ok
test importer::tests::validate_initial_stock_requires_quantity_and_sku_or_barcode_mapping ... ok
```
(total importer `#[test]` count: 10, all `ok`).

- [ ] **Step 3 — Write the frontend drill-down test (genuinely fails first).** Add this `it(...)` inside the existing import `describe` block in `/Users/adnan/Projects/vantumpos/src/App.test.tsx`, right after the `"commits a valid product CSV after dry run confirmation"` test closes at line `702` (before the block-closing `});` at `703`). It reuses the existing `openImportModule(user)` helper (`App.test.tsx:705-722`) and the already-imported `within`:

```tsx
  it("opens the import job detail drill-down from history", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await openImportModule(user);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji importa prethodni-products.csv",
      }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: "Detalji importa: prethodni-products.csv",
    });
    expect(within(dialog).getByText("Red 2")).toBeInTheDocument();
    expect(within(dialog).getByText("Upisano")).toBeInTheDocument();
  });
```
(The seed job `prethodni-products.csv` is mock-adapter.ts:122-133; `openImportModule` logs in as `admin` and clicks the `Import` nav button.)

- [ ] **Step 4 — Run the frontend test, expect FAIL.**
```
cd /Users/adnan/Projects/vantumpos && bunx vitest run src/App.test.tsx
```
Expected failure: `TestingLibraryElementError: Unable to find role="button" and name "Detalji importa prethodni-products.csv"` — the history file cell is plain text today (`ImportWizard.tsx:526`), not a button.

- [ ] **Step 5 — Make the mock return inspectable rows.** Edit `/Users/adnan/Projects/vantumpos/src/services/mock-adapter.ts:716-727`. Replace the `rows: []` stub so `getImportJob` returns a representative imported row:

```ts
      async getImportJob(id) {
        const job = importJobs.find((item) => item.id === id);

        if (!job) {
          return null;
        }

        return {
          ...job,
          rows: [
            {
              rowNumber: 2,
              status: "imported" as const,
              message: null,
              values: { Naziv: "Hleb", Cena: "120,00" },
            },
          ],
        };
      },
```

- [ ] **Step 6 — Add the Dialog import to ImportWizard.** In `/Users/adnan/Projects/vantumpos/src/app/import/ImportWizard.tsx`, insert a new import block after the `card` import block (after line `35`, before the `empty` import at line `36`):

```tsx
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
```
Then extend the `@/services/types` type import (currently `ImportWizard.tsx:69-75`) to include `ImportJobDetail`:

```tsx
import type {
  ImportHeaders,
  ImportJob,
  ImportJobDetail,
  ImportMapping,
  ImportType,
  ImportValidationResult,
} from "@/services/types";
```

- [ ] **Step 7 — Thread `services` into `ImportHistory` at the call site.** Change `ImportWizard.tsx:425` from:
```tsx
      <ImportHistory history={history} />
```
to:
```tsx
      <ImportHistory history={history} services={services} />
```

- [ ] **Step 8 — Make history rows clickable + render the detail dialog.** Replace the entire `ImportHistory` function (`ImportWizard.tsx:505-555`) with the version below. It adds local detail state, calls `services.imports.getImportJob(job.id)`, turns the file cell into a `link`-variant `Button` (already a valid variant, `button.tsx:20`), and renders a new `ImportJobDetailDialog`. It reuses the existing `messageFromError` (`ImportWizard.tsx:623`), `statusLabel` (`ImportWizard.tsx:595`), `IMPORT_TYPE_LABELS` (`ImportWizard.tsx:81`), `Badge`, `Empty*`, `Alert*`, `Table*`, and `FileTextIcon`/`HistoryIcon` already imported in this file:

```tsx
function ImportHistory({
  history,
  services,
}: {
  history: ImportJob[];
  services: PosServices;
}) {
  const [detail, setDetail] = useState<ImportJobDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailBusy, setDetailBusy] = useState(false);

  async function openDetail(jobId: number) {
    setDetailBusy(true);
    setDetailError(null);

    try {
      const job = await services.imports.getImportJob(jobId);

      if (job) {
        setDetail(job);
      } else {
        setDetailError("Detalji importa nisu dostupni.");
      }
    } catch (caught) {
      setDetailError(messageFromError(caught));
    } finally {
      setDetailBusy(false);
    }
  }

  function closeDetail(open: boolean) {
    if (!open) {
      setDetail(null);
      setDetailError(null);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Istorija importa</CardTitle>
        <CardDescription>Poslednji upisi podataka u lokalnu bazu.</CardDescription>
      </CardHeader>
      <CardContent>
        {history.length ? (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Fajl</TableHead>
                <TableHead>Tip</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Redovi</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {history.map((job) => (
                <TableRow key={job.id}>
                  <TableCell>
                    <Button
                      type="button"
                      variant="link"
                      className="h-auto p-0"
                      aria-label={`Detalji importa ${job.fileName}`}
                      disabled={detailBusy}
                      onClick={() => openDetail(job.id)}
                    >
                      {job.fileName}
                    </Button>
                  </TableCell>
                  <TableCell>{IMPORT_TYPE_LABELS[job.importType]}</TableCell>
                  <TableCell>
                    <Badge variant={job.status === "failed" ? "destructive" : "outline"}>
                      {job.status === "completed" ? "Zavrsen" : job.status}
                    </Badge>
                  </TableCell>
                  <TableCell>{job.totalRows}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HistoryIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema prethodnih import poslova</EmptyTitle>
              <EmptyDescription>
                Prvi uspesan upis ce se pojaviti ovde.
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent />
          </Empty>
        )}
      </CardContent>
      <ImportJobDetailDialog
        detail={detail}
        error={detailError}
        onOpenChange={closeDetail}
      />
    </Card>
  );
}

function ImportJobDetailDialog({
  detail,
  error,
  onOpenChange,
}: {
  detail: ImportJobDetail | null;
  error: string | null;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={Boolean(detail || error)} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {detail ? `Detalji importa: ${detail.fileName}` : "Detalji importa"}
          </DialogTitle>
          <DialogDescription>
            Redovi importa sa statusom i porukom za svaki red.
          </DialogDescription>
        </DialogHeader>

        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>Detalji nisu dostupni</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : detail && detail.rows.length ? (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Red</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Poruka</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {detail.rows.map((row) => (
                <TableRow key={row.rowNumber}>
                  <TableCell>Red {row.rowNumber}</TableCell>
                  <TableCell>
                    <Badge
                      variant={row.status === "error" ? "destructive" : "secondary"}
                    >
                      {statusLabel(row.status)}
                    </Badge>
                  </TableCell>
                  <TableCell>{row.message || "-"}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <FileTextIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema sacuvanih redova</EmptyTitle>
              <EmptyDescription>Ovaj posao nema redove za prikaz.</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 9 — Run the frontend test, expect PASS.**
```
cd /Users/adnan/Projects/vantumpos && bunx vitest run src/App.test.tsx
```
Expected: `✓ src/App.test.tsx` with `opens the import job detail drill-down from history` passing, and the two pre-existing import tests (`validates imported product rows before enabling commit`, `commits a valid product CSV after dry run confirmation`) still green.

- [ ] **Step 10 — Correct the docs miscount.** In `/Users/adnan/Projects/vantumpos/docs/PROGRESS.md` update the import-module coverage line (`572`) and the auditor over-claim note (`574`) to reflect the now-present tests. Replace at line `572` the leading `Backend (7) —` count with `Backend (10) —`, append `, unknown-VAT row error, known-VAT row accepted, initial-stock required-mapping` to its test list, and delete the now-resolved gaps `no unknown-VAT-row test;` and `no initial-stock validation-failure test;`. At line `574` change the over-claim sentence to read that the "8 backend tests" bullet was a miscount and the suite now has **10** `#[test]` functions after adding the spec-named unknown-VAT and initial-stock required-mapping tests.

- [ ] **Step 11 — Run the full verification gates.**
```
cd /Users/adnan/Projects/vantumpos && bun run test && bun run build
cd /Users/adnan/Projects/vantumpos/src-tauri && cargo test -- --test-threads=1 && cargo clippy --all-targets --all-features --locked -- -D warnings && cargo fmt --check
```
Expected: all suites green, `build` succeeds, clippy clean (`-D warnings`), `cargo fmt --check` reports no diffs.

- [ ] **Final step — Commit.**
```
cd /Users/adnan/Projects/vantumpos && git add \
  src-tauri/src/importer.rs \
  src/App.test.tsx \
  src/services/mock-adapter.ts \
  src/app/import/ImportWizard.tsx \
  docs/PROGRESS.md && \
git commit -m "$(cat <<'EOF'
feat(import): job-detail drill-down + unknown-VAT/required-mapping tests

Consume import_get_job in the Import history table via a per-row detail
dialog; add backend unknown-VAT (fail + resolve) and initial-stock
required-mapping validation tests; correct the import test-count in docs.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

I have all the ground truth confirmed. Here is the task section.

### Task 11: C1 (rec #10) — Settings/Backup component tests + age-based stale detection

**Files:**
- Create (test): `/Users/adnan/Projects/vantumpos/src/app/settings/SettingsScreen.test.tsx`
- Modify: `/Users/adnan/Projects/vantumpos/src-tauri/src/commands/backup.rs` — add a `BACKUP_STALE_AFTER_HOURS` constant after line 12, replace the `stale:` field computation inside `load_backup_status` (lines 125-140), add a private `is_backup_stale` helper, and add two `#[test]` fns inside the existing `mod tests` (after line 481).

**Interfaces:**
- Consumes (unchanged, already in repo):
  - `SettingsScreen({ services, usersPanel }: { services: PosServices; usersPanel: ReactNode })` — `src/app/settings/SettingsScreen.tsx:87`.
  - `createMockServices(): PosServices & { initialSession: AuthSession | null }` — `src/services/mock-adapter.ts:29`.
  - `Toaster` — `@/components/ui/sonner`.
  - `with_state(test_name, |state: &AppState|)` and `state.db().open()` (`pub fn db(&self) -> &Db`, `state.rs:23`), test DB helper `Db::new` — per shared ground truth.
  - Rust DTOs `BackupStatus`/`BackupJob` (`backup.rs:30-51`), TS types `BackupStatus`/`BackupJob` (`types.ts:72-87`).
- Produces (new, internal to this task):
  - Rust const `BACKUP_STALE_AFTER_HOURS: i64 = 24` and `fn is_backup_stale(state: &AppState) -> Result<bool, AppError>` in `backup.rs`. No public API/signature changes — `BackupStatus.stale` semantics change from "never backed up" to "no successful backup within 24h". No frontend type or adapter change required (FE already reads `status.stale`).

TDD steps:

- [ ] **Step 1: Write the failing backend test (age-based staleness).** Append these two tests inside the existing `mod tests` in `/Users/adnan/Projects/vantumpos/src-tauri/src/commands/backup.rs`, after the closing `}` of `restore_requires_confirmation_text` (line 481). They use the in-file `with_state` helper (backup.rs:378) and insert `backup_jobs` rows directly with backdated `created_at` via SQLite's `datetime('now', '-N hours')` (same fixed `YYYY-MM-DD HH:MM:SS` format the production insert at `backup.rs:289` uses, so the string comparison is valid):

  ```rust
      #[test]
      fn backup_status_is_stale_when_last_success_is_older_than_threshold() {
          with_state(
              "backup_status_is_stale_when_last_success_is_older_than_threshold",
              |state| {
                  let conn = state.db().open().expect("database should open");
                  conn.execute(
                      "INSERT INTO backup_jobs (
                          backup_type, path, status, error_message,
                          file_size_bytes, created_at, completed_at
                       )
                       VALUES (
                          'manual', '/tmp/vantumpos-old.sqlite3', 'completed', NULL, 1024,
                          datetime('now', '-48 hours'), datetime('now', '-48 hours')
                       )",
                      [],
                  )
                  .expect("aged backup job should insert");

                  let status = load_backup_status(state).expect("backup status should load");

                  assert!(status.last_successful_backup.is_some());
                  assert!(status.stale);
              },
          );
      }

      #[test]
      fn backup_status_is_fresh_when_recent_success_exists() {
          with_state(
              "backup_status_is_fresh_when_recent_success_exists",
              |state| {
                  let conn = state.db().open().expect("database should open");
                  conn.execute(
                      "INSERT INTO backup_jobs (
                          backup_type, path, status, error_message,
                          file_size_bytes, created_at, completed_at
                       )
                       VALUES (
                          'manual', '/tmp/vantumpos-recent.sqlite3', 'completed', NULL, 1024,
                          datetime('now', '-1 hours'), datetime('now', '-1 hours')
                       )",
                      [],
                  )
                  .expect("recent backup job should insert");

                  let status = load_backup_status(state).expect("backup status should load");

                  assert!(status.last_successful_backup.is_some());
                  assert!(!status.stale);
              },
          );
      }
  ```

- [ ] **Step 2: Run it, expect FAIL.** Command:
  `cd /Users/adnan/Projects/vantumpos/src-tauri && cargo test --lib backup_status_is -- --test-threads=1`
  Expected: `backup_status_is_fresh_when_recent_success_exists ... ok` but `backup_status_is_stale_when_last_success_is_older_than_threshold ... FAILED`, with a panic `assertion failed: status.stale`. Reason: the current `load_backup_status` (backup.rs:136) sets `stale: last_successful_backup.is_none()`; the 48h-old job makes `last_successful_backup` `Some`, so `stale` is wrongly `false`.

- [ ] **Step 3: Implement age-based staleness in `backup.rs`.** Add the threshold constant after `RESTORE_CONFIRMATION` (line 12):

  ```rust
  const RESTORE_CONFIRMATION: &str = "VRATI PODATKE";
  const BACKUP_STALE_AFTER_HOURS: i64 = 24;
  ```

  Replace the `stale:` line inside `load_backup_status` (backup.rs:125-140). Change the body so it reads:

  ```rust
  pub fn load_backup_status(state: &AppState) -> Result<BackupStatus, AppError> {
      let settings = load_backup_settings(state)?;
      let last_successful_backup = query_latest_backup_job(
          state,
          "status = 'completed' AND backup_type IN ('manual', 'automatic')",
      )?;
      let last_failed_backup = query_latest_backup_job(state, "status = 'failed'")?;
      let stale = is_backup_stale(state)?;

      Ok(BackupStatus {
          backup_folder: settings.backup_folder,
          automatic_backup_enabled: settings.automatic_backup_enabled,
          stale,
          last_successful_backup,
          last_failed_backup,
      })
  }
  ```

  Add the helper immediately after `load_backup_status` (before `create_backup`). `params` is already imported (backup.rs:4); SQLite `EXISTS` yields 0/1 which rusqlite maps to `bool`:

  ```rust
  fn is_backup_stale(state: &AppState) -> Result<bool, AppError> {
      let conn = state.db().open()?;
      let threshold_modifier = format!("-{BACKUP_STALE_AFTER_HOURS} hours");
      let has_recent_backup: bool = conn.query_row(
          "SELECT EXISTS(
              SELECT 1
              FROM backup_jobs
              WHERE status = 'completed'
                AND backup_type IN ('manual', 'automatic')
                AND created_at >= datetime('now', ?1)
           )",
          params![threshold_modifier],
          |row| row.get(0),
      )?;

      Ok(!has_recent_backup)
  }
  ```

  Note: when there is no successful backup at all, `has_recent_backup` is `false` so `stale` stays `true`, preserving the original `is_none` behavior; the existing tests `manual_backup_creates_sqlite_copy_and_success_job` and `backup_settings_round_trip_drives_status` do not assert on `stale`, so they remain green.

- [ ] **Step 4: Run it, expect PASS.** Command:
  `cd /Users/adnan/Projects/vantumpos/src-tauri && cargo test --lib backup_status_is -- --test-threads=1`
  Expected: both `backup_status_is_stale_when_last_success_is_older_than_threshold ... ok` and `backup_status_is_fresh_when_recent_success_exists ... ok`; `test result: ok.`

- [ ] **Step 5: Write the frontend `SettingsScreen.test.tsx`.** Create `/Users/adnan/Projects/vantumpos/src/app/settings/SettingsScreen.test.tsx`. It renders the component directly (per the C1 work unit, not via `AppShell`) with a sibling `<Toaster />` for toast assertions (mirroring `ReportsScreen.test.tsx:109-120`) and injects the shared mock (`mock-adapter.ts:29`). These cover the five spec-named frontend tests from module 08 (`docs/module-specs/08-settings-backup.md:154-160`): tabs render distinct content; company save + toast; VAT validation; backup stale/failed render; restore confirmation required.

  ```tsx
  import { render, screen, within } from "@testing-library/react";
  import userEvent from "@testing-library/user-event";
  import { describe, expect, it } from "vitest";

  import { SettingsScreen } from "./SettingsScreen";
  import { Toaster } from "@/components/ui/sonner";
  import { createMockServices } from "@/services/mock-adapter";
  import type { BackupJob, BackupStatus } from "@/services/types";

  function renderSettings(services = createMockServices()) {
    render(
      <>
        <SettingsScreen
          services={services}
          usersPanel={<div>Korisnici panel</div>}
        />
        <Toaster />
      </>,
    );
    return services;
  }

  describe("SettingsScreen", () => {
    it("renders distinct content for each settings tab", async () => {
      const user = userEvent.setup();
      renderSettings();

      expect(await screen.findByLabelText("Naziv radnje")).toBeInTheDocument();

      await user.click(screen.getByRole("tab", { name: "PDV" }));
      expect(
        await screen.findByRole("heading", { name: "PDV stope" }),
      ).toBeInTheDocument();

      await user.click(screen.getByRole("tab", { name: "Racuni" }));
      expect(screen.getByLabelText("Prefiks racuna")).toBeInTheDocument();

      await user.click(screen.getByRole("tab", { name: "Korisnici" }));
      expect(screen.getByText("Korisnici panel")).toBeInTheDocument();

      await user.click(screen.getByRole("tab", { name: "Backup" }));
      expect(
        await screen.findByRole("heading", { name: "Status backupa" }),
      ).toBeInTheDocument();
    });

    it("saves company settings and shows a success toast", async () => {
      const user = userEvent.setup();
      renderSettings();

      await user.clear(await screen.findByLabelText("Naziv radnje"));
      await user.type(screen.getByLabelText("Naziv radnje"), "Vantum Market");
      await user.click(screen.getByRole("button", { name: "Sacuvaj radnju" }));

      expect(
        await screen.findByText("Podesavanja radnje su sacuvana."),
      ).toBeInTheDocument();
    });

    it("renders a VAT validation error without closing the dialog", async () => {
      const user = userEvent.setup();
      renderSettings();

      await user.click(screen.getByRole("tab", { name: "PDV" }));
      await user.click(await screen.findByRole("button", { name: "Nova PDV stopa" }));
      await user.click(screen.getByRole("button", { name: "Sacuvaj PDV stopu" }));

      expect(
        await screen.findByText("Naziv PDV stope je obavezan."),
      ).toBeInTheDocument();
      expect(screen.getByRole("heading", { name: "PDV stopa" })).toBeInTheDocument();
    });

    it("renders the stale warning and a failed backup job", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const failedJob: BackupJob = {
        id: 99,
        backupType: "automatic",
        path: "D:/backups/vantumpos-automatic-99.sqlite3",
        status: "failed",
        errorMessage: "Disk pun.",
        fileSizeBytes: null,
        createdAt: "2026-06-25 10:00:00",
        completedAt: null,
      };
      const staleStatus: BackupStatus = {
        backupFolder: "D:/backups",
        automaticBackupEnabled: true,
        stale: true,
        lastSuccessfulBackup: null,
        lastFailedBackup: failedJob,
      };
      services.backup.getBackupStatus = async () => staleStatus;
      services.backup.listBackupJobs = async () => [failedJob];

      renderSettings(services);

      await user.click(await screen.findByRole("tab", { name: "Backup" }));

      expect(await screen.findByText("Backup nije napravljen")).toBeInTheDocument();

      const historyRow = screen.getByText(failedJob.path).closest("tr");
      expect(historyRow).not.toBeNull();
      expect(
        within(historyRow as HTMLElement).getByText("Neuspesan"),
      ).toBeInTheDocument();
    });

    it("requires the exact confirmation text before restore", async () => {
      const user = userEvent.setup();
      renderSettings();

      await user.click(await screen.findByRole("tab", { name: "Backup" }));
      await user.type(
        screen.getByLabelText("Putanja backup fajla"),
        "D:/backup.sqlite3",
      );
      await user.click(screen.getByRole("button", { name: "Vrati backup" }));

      expect(
        screen.getByRole("heading", { name: "Potvrdite restore" }),
      ).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Potvrdi restore" })).toBeDisabled();

      await user.type(screen.getByLabelText("Potvrda"), "VRATI PODATKE");
      expect(screen.getByRole("button", { name: "Potvrdi restore" })).toBeEnabled();
    });
  });
  ```

  These tests assert against existing component behavior (`SettingsScreen.tsx` tabs at lines 149-178, company toast at 189, VAT dialog at 538-540, stale alert at 762-769 / `Neuspesan` badge at 870-872, restore guard at 905-907), so they close the module-08 frontend test gap. The "stale/failed render" test overrides `services.backup.getBackupStatus`/`listBackupJobs` (the "mutate the mock after creation" style, `App.test.tsx:365-368`) because the default mock's `createBackupJob` only emits `status: "completed"` (mock-adapter.ts:879).

- [ ] **Step 6: Run the frontend tests, expect PASS.** Command:
  `cd /Users/adnan/Projects/vantumpos && bunx vitest run src/app/settings/SettingsScreen.test.tsx`
  Expected: `Test Files  1 passed (1)` and `Tests  5 passed (5)`. (The component already satisfies module-08 behavior, so these gap-closing characterization tests pass without any UI change; the genuine red→green cycle for this task is the backend change in Steps 1-4.)

- [ ] **Step 7: Run full gates.** Commands and expected:
  - `cd /Users/adnan/Projects/vantumpos && bun run test` → all suites pass (existing `App.test.tsx` Podesavanja tests at lines 234-308 stay green; new `SettingsScreen.test.tsx` green).
  - `cd /Users/adnan/Projects/vantumpos && bun run build` → builds with no TS errors.
  - `cd /Users/adnan/Projects/vantumpos/src-tauri && cargo test -- --test-threads=1` → `test result: ok.` including the two new `commands::backup::tests` cases.
  - `cd /Users/adnan/Projects/vantumpos/src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings` → no warnings (inline-arg `format!("-{BACKUP_STALE_AFTER_HOURS} hours")` is clippy-clean).
  - `cd /Users/adnan/Projects/vantumpos/src-tauri && cargo fmt --check` → no diff.

- [ ] **Final step: Commit.** Command:
  `cd /Users/adnan/Projects/vantumpos && git add src/app/settings/SettingsScreen.test.tsx src-tauri/src/commands/backup.rs && git commit -m "$(cat <<'EOF'
  test(settings): add SettingsScreen component tests and age-based backup staleness

  Add SettingsScreen.test.tsx covering tab content, company save toast, VAT
  validation, stale/failed backup render, and guarded restore confirmation.
  Make backup staleness time-based (no successful manual/automatic backup
  within 24h) instead of is_none-only in commands/backup.rs.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"`

---

### Task 12: C2 (rec #11) — Shell header company name + migration regression test

**Files:**
- Modify: `src/app/AppShell.tsx` (add `companyName` state after `useState<SessionState>` block at lines 124-128; add a company-settings `useEffect` after the session effect ending at line 158; replace the hard-coded header literal at line 208)
- Test (frontend): `src/App.test.tsx` (add a module-level `readyCompany` fixture near `readyHealth` at lines 11-16; extend `buildAuthServices`'s `settings` slice at lines 20-22; add two new `it(...)` cases in the `describe("AppShell", ...)` block)
- Test + Modify (Rust): `src-tauri/src/db/migrations.rs` (append a `#[cfg(test)] mod tests` after `run_migrations` ends at line 287; no production-code change to the migration list)

**Interfaces:**
- Consumes (no Task 2 role/session helper needed here):
  - `SettingsService.getCompanySettings(): Promise<CompanySettings>` (`src/services/ports.ts:64`), where `CompanySettings.shopName: string` (`src/services/types.ts:18-26`). The mock returns `shopName: "Vantum Market"` (`src/services/mock-adapter.ts:134-142`).
  - `run_migrations(conn: &mut Connection) -> Result<(), AppError>` (`src-tauri/src/db/migrations.rs:256`); the `const MIGRATIONS: &[Migration]` slice (`migrations.rs:11`) with fields `version: i64`, `name: &'static str`, `sql: &'static str`; `crate::db::test_database_path(test_name: &str) -> PathBuf` (`src-tauri/src/db/mod.rs:88`).
- Produces:
  - Internal `companyName` shell state driving the sidebar brand (no new exported API).
  - Rust test `old_database_migrates_forward_to_latest_schema` in `migrations.rs` (guards the installed-base upgrade path for all later schema work).

---

- [ ] **Step 1: Write the failing frontend tests.** In `src/App.test.tsx`, add the `readyCompany` fixture immediately after the existing `readyHealth` const (lines 11-16):

```tsx
const readyCompany = {
  shopName: "Vantum Market",
  address: "Bulevar 1, Beograd",
  pib: "",
  registrationNumber: "87654321",
  phone: "+381 11 123 456",
  logoPath: null,
  currency: "RSD" as const,
};
```

Then add these two cases inside `describe("AppShell", ...)` (e.g. right after the `"renders the POS navigation and backend status"` test):

```tsx
  it("shows the company shop name as the shell brand", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(await screen.findByText("Vantum Market")).toBeInTheDocument();
    expect(screen.queryByText("VantumPOS")).not.toBeInTheDocument();
  });

  it("keeps default branding when company settings cannot be read", async () => {
    const getCompanySettings = vi.fn().mockRejectedValue({
      code: "database_error",
      message: "Baza nije dostupna.",
    });
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(adminSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
      settings: {
        getHealth: vi.fn().mockResolvedValue(readyHealth),
        getCompanySettings,
      },
    });

    render(<AppShell services={services} />);

    expect(await screen.findByText("Administrator")).toBeInTheDocument();
    await waitFor(() => expect(getCompanySettings).toHaveBeenCalled());
    expect(screen.getByText("VantumPOS")).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run the frontend tests, expect FAIL.**
  - Command: `bunx vitest run src/App.test.tsx`
  - Expected: the happy-path test fails with `Unable to find an element with the text: Vantum Market` (the header is still the hard-coded literal); the failure-path test fails on `expected getCompanySettings to have been called` (the shell never calls it yet). The pre-existing `buildAuthServices` tests still pass because `getCompanySettings` is not yet invoked by the shell.

- [ ] **Step 3: Implement the shell wiring.** In `src/app/AppShell.tsx`:

  (a) Add the `companyName` state. Replace:
```tsx
  );
  const activeItem =
    navigationItems.find((item) => item.id === activeId) ?? navigationItems[0];
```
  with:
```tsx
  );
  const [companyName, setCompanyName] = useState("VantumPOS");
  const activeItem =
    navigationItems.find((item) => item.id === activeId) ?? navigationItems[0];
```

  (b) Add the company-settings effect. Replace the end of the session effect:
```tsx
    return () => {
      ignore = true;
    };
  }, [initialSession, services]);
```
  with:
```tsx
    return () => {
      ignore = true;
    };
  }, [initialSession, services]);

  useEffect(() => {
    let ignore = false;

    services.settings
      .getCompanySettings()
      .then((company) => {
        if (ignore) {
          return;
        }
        const name = company.shopName.trim();
        setCompanyName(name.length > 0 ? name : "VantumPOS");
      })
      .catch(() => {
        // Keep the default branding if company settings cannot be read.
      });

    return () => {
      ignore = true;
    };
  }, [services]);
```

  (c) Bind the sidebar header to state. Replace (unique via the `group-data-[collapsible=icon]:hidden` / `text-sidebar-foreground/70` context — this is the sidebar header, not the login header at line 482):
```tsx
              <div className="min-w-0 group-data-[collapsible=icon]:hidden">
                <div className="truncate text-sm font-medium">VantumPOS</div>
                <div className="truncate text-xs text-sidebar-foreground/70">
```
  with:
```tsx
              <div className="min-w-0 group-data-[collapsible=icon]:hidden">
                <div className="truncate text-sm font-medium">{companyName}</div>
                <div className="truncate text-xs text-sidebar-foreground/70">
```

  (d) Keep the existing `buildAuthServices` shell tests green (the shell now calls `getCompanySettings` on every mount; the hand-rolled mock omits it). In `src/App.test.tsx`, extend the `settings` slice of `buildAuthServices` (lines 20-22). Replace:
```tsx
    settings: {
      getHealth: vi.fn().mockResolvedValue(readyHealth),
    },
```
  with:
```tsx
    settings: {
      getHealth: vi.fn().mockResolvedValue(readyHealth),
      getCompanySettings: vi.fn().mockResolvedValue(readyCompany),
    },
```

- [ ] **Step 4: Run the frontend tests, expect PASS.**
  - Command: `bunx vitest run src/App.test.tsx`
  - Expected: all `AppShell` cases pass, including the two new ones. Then run the full suite `bun run test` and the type/build gate `bun run build` — both green.

- [ ] **Step 5: Write the failing Rust migration regression test.** Append to `src-tauri/src/db/migrations.rs` (after the closing `}` of `run_migrations` at line 287):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_database_path;

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info should prepare");
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("table_info should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("table_info rows should collect");
        names.iter().any(|name| name == column)
    }

    #[test]
    fn old_database_migrates_forward_to_latest_schema() {
        let path = test_database_path("migrations_forward");

        {
            let mut conn = Connection::open(&path).expect("connection should open");

            // Simulate an installed database stuck at schema version 1 only.
            conn.execute_batch(
                r#"
CREATE TABLE _migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);
"#,
            )
            .expect("migrations table should create");
            conn.execute_batch(MIGRATIONS[0].sql)
                .expect("version 1 schema should apply");
            conn.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                params![MIGRATIONS[0].version, MIGRATIONS[0].name],
            )
            .expect("version 1 should record");

            // Columns added by later migrations must be absent before the upgrade.
            assert!(!column_exists(&conn, "users", "last_login_at"));
            assert!(!column_exists(&conn, "sales", "document_type"));
            assert!(!column_exists(&conn, "products", "external_source_provider"));

            // The real installed-base upgrade path.
            run_migrations(&mut conn).expect("forward migration should succeed");

            let applied: i64 = conn
                .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
                .expect("migration count should query");
            assert_eq!(applied, MIGRATIONS.len() as i64);

            assert!(column_exists(&conn, "users", "last_login_at"));
            assert!(column_exists(&conn, "sales", "document_type"));
            assert!(column_exists(&conn, "products", "external_source_provider"));

            // Re-running migrations on an up-to-date database is a no-op.
            run_migrations(&mut conn).expect("re-running migrations should be a no-op");
            let applied_again: i64 = conn
                .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
                .expect("migration count should query");
            assert_eq!(applied_again, MIGRATIONS.len() as i64);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }
}
```

- [ ] **Step 6: Run the Rust test, expect PASS (and confirm it would fail on a regression).**
  - Command: `cargo test --manifest-path /Users/adnan/Projects/vantumpos/src-tauri/Cargo.toml old_database_migrates_forward_to_latest_schema -- --test-threads=1`
  - Expected: `test db::migrations::tests::old_database_migrates_forward_to_latest_schema ... ok` (`1 passed`). This test is RED for any future change that breaks forward migration (e.g. an `ALTER TABLE` that fails on the v1 schema, or a migration that omits its `_migrations` insert) — the `run_migrations(...).expect("forward migration should succeed")` call or the `assert_eq!(applied, MIGRATIONS.len() as i64)` will fail. To sanity-check the guard locally, temporarily break a later migration's SQL and observe the failure, then revert.

- [ ] **Step 7: Run all five verification gates, expect PASS.**
  - `bun run test`
  - `bun run build`
  - `cd src-tauri && cargo test -- --test-threads=1`
  - `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cd src-tauri && cargo fmt --check`
  - Expected: all green; no clippy warnings; `fmt --check` reports no diffs.

- [ ] **Final step: Commit.**
  - `git add src/app/AppShell.tsx src/App.test.tsx src-tauri/src/db/migrations.rs`
  - Commit message:
    ```
    feat: read shell brand from company settings + migration regression test

    Wire the sidebar header to settings_get_company so the configured shop
    name replaces the hard-coded "VantumPOS" brand, falling back to the
    default when settings are unreadable. Add a forward-migration regression
    test that upgrades a version-1 database to the latest schema and asserts
    later-migration columns exist and re-running is a no-op.

    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
    ```

---

### Task 13: Final verification, adversarial review & PROGRESS.md update

**Files:**
- Modify: `docs/PROGRESS.md`
- (no new source changes expected beyond addressing review findings)

**Interfaces:**
- Consumes: the completed work of Tasks 1–12.
- Produces: an updated progress report reflecting MVP completion.

- [ ] **Step 1: Run the full gate suite**

```bash
bun run test
bun run build
cd src-tauri && cargo test -- --test-threads=1
cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings
cd src-tauri && cargo fmt --check
```
Expected: all five pass with zero failures/warnings.

- [ ] **Step 2: Confirm no hard-coded operator id remains**

Run: `rg "userId: 1" src/`
Expected: no matches.

- [ ] **Step 3: Confirm admin role-gating is enforced in Rust**

Run: `cd src-tauri && cargo test settings_update_receipt -- --test-threads=1`
Expected: PASS, including the non-admin-rejected case.

- [ ] **Step 4: Adversarial code-review pass**

Review the full branch diff (`git diff master...HEAD`) with the `superpowers:requesting-code-review` skill (Opus). Address any correctness/compliance findings before proceeding.

- [ ] **Step 5: Update `docs/PROGRESS.md`**

Update each module's completion %, status, Definition-of-Done checklist, and the build/test counts to reflect the completed work units. Remove now-closed gaps from "Recommended Next Steps".

- [ ] **Step 6: Commit**

```bash
git add docs/PROGRESS.md
git commit -m "docs: update progress report to reflect MVP completion"
```
