# Module Spec: Auth, Users, And Shifts

## Goal

Implement the identity and shift layer needed before real selling can happen. The cashier must sign in, open a shift, and close it with counted cash. Admin users must be manageable locally.

## Scope

In scope:

- Login screen with username plus password or PIN.
- Session state in the running app.
- User list and user create/edit/deactivate flow for admins.
- Open shift flow with opening cash amount.
- Close shift flow with expected cash/card totals and counted cash input.
- Header/footer shell state backed by auth and shift services.

Out of scope:

- Cloud accounts.
- Password reset email.
- OS-level secure credential store.
- Multi-register shift sharing.

## Screens

### Login

Use a compact auth layout inspired by shadcn auth blocks, but keep it desktop-tool oriented. Fields:

- username,
- PIN or password,
- submit button,
- visible error area.

After successful login:

- if user has open shift, land on Kasa,
- if cashier has no open shift, show Open Shift,
- if admin has no open shift, allow navigation but sale completion still requires shift.

### Open Shift

Fields:

- opening cash,
- note optional.

Primary action: `Otvori smenu`.

### Close Shift

Show:

- opened at,
- cashier,
- cash expected,
- card total,
- counted cash input,
- difference,
- note.

Primary action: `Zatvori smenu`. Use `AlertDialog` for confirmation.

### Users

This can live under Podesavanja later, but the service should be ready here.

Use `Table` for users and `Dialog` or `Sheet` for edit form.

## Service Contract

Add to `src/services/ports.ts`:

```ts
interface AuthService {
  getSession(): Promise<AppSession | null>
  login(request: LoginRequest): Promise<AppSession>
  logout(): Promise<void>
}

interface UsersService {
  listUsers(): Promise<UserSummary[]>
  createUser(request: SaveUserRequest): Promise<UserSummary>
  updateUser(id: number, request: SaveUserRequest): Promise<UserSummary>
  deactivateUser(id: number): Promise<void>
}

interface ShiftService {
  getCurrentShift(): Promise<ShiftSummary | null>
  openShift(request: OpenShiftRequest): Promise<ShiftSummary>
  closeShift(request: CloseShiftRequest): Promise<ShiftSummary>
}
```

## Backend Commands

- `auth_get_session`
- `auth_login`
- `auth_logout`
- `users_list`
- `users_create`
- `users_update`
- `users_deactivate`
- `shift_get_current`
- `shift_open`
- `shift_close`

For MVP, session can be process-local in Tauri state. Persisted remember-me is not required.

## SQLite Notes

Existing tables: `users`, `shifts`.

Add a migration if needed for:

- password hash salt metadata,
- last login timestamp,
- shift closing note.

PIN/password hashing must happen in Rust. If a hashing dependency is added, document the choice in the implementation plan and test invalid credentials.

## Business Rules

- Deactivated users cannot log in.
- Only one open shift per user.
- A cashier cannot complete a sale without an open shift.
- Closing shift requires counted cash.
- Expected cash is derived from completed cash payments minus local cash refunds/voids.
- Closing a shift does not delete or mutate sales.

## Tests

Backend:

- login succeeds with valid credentials,
- login fails for invalid credentials,
- deactivated user cannot log in,
- opening a second shift for the same user fails,
- closing shift stores counted cash and status `closed`.

Frontend:

- login success routes to the next required state,
- invalid login shows Serbian message,
- open shift form rejects invalid money,
- shell shows real user and shift status after login.

## Acceptance Criteria

- App no longer shows hard-coded `Admin` or `Smena nije otvorena` as fixed text.
- User can sign in, open a shift, close it, and see shell status update.
- Kasa sale completion can later depend on `shiftService.getCurrentShift`.

## Prompt For Separate Chat

```text
Implementiraj Auth, Users, and Shifts module for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/01-auth-shifts.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Do not implement sales or catalog beyond what is needed for shift totals. Replace hard-coded shell user/shift state with real service-backed state. Add Rust/Tauri commands, TS service contracts, mock adapter data, UI screens, and tests.
```

