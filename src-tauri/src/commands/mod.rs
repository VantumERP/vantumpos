pub mod auth;
pub mod backup;
pub mod backup_crypto;
pub mod campaigns;
pub mod catalog;
pub mod health;
pub mod imports;
pub mod inventory;
pub mod receipts;
pub mod reklamacije;
pub mod reports;
pub mod sales;
pub mod settings;
pub mod shifts;
pub mod users;

#[cfg(test)]
mod auth_shift_tests {
    use crate::commands::auth::{login_user, LoginRequest};
    use crate::commands::shifts::{
        close_shift_for_user, open_shift_for_user, CloseShiftRequest, OpenShiftRequest,
    };
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    #[test]
    fn login_succeeds_with_valid_credentials() {
        let db_path = test_database_path("login_succeeds_with_valid_credentials");
        let db = Db::new(&db_path).expect("database should initialize");
        let state = AppState::new(db);

        let session = login_user(
            &state,
            LoginRequest {
                username: "admin".into(),
                credential: "1234".into(),
            },
        )
        .expect("admin should log in");

        assert_eq!(session.user.username, "admin");

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn login_fails_for_invalid_credentials() {
        let db_path = test_database_path("login_fails_for_invalid_credentials");
        let db = Db::new(&db_path).expect("database should initialize");
        let state = AppState::new(db);

        let error = login_user(
            &state,
            LoginRequest {
                username: "admin".into(),
                credential: "wrong".into(),
            },
        )
        .expect_err("invalid login should fail");

        assert_eq!(error.code, "invalid_credentials");

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn opening_second_shift_for_same_user_fails() {
        let db_path = test_database_path("opening_second_shift_for_same_user_fails");
        let db = Db::new(&db_path).expect("database should initialize");
        let state = AppState::new(db);
        let session = login_user(
            &state,
            LoginRequest {
                username: "admin".into(),
                credential: "1234".into(),
            },
        )
        .expect("admin should log in");

        open_shift_for_user(
            &state,
            session.user.id,
            OpenShiftRequest {
                opening_cash_minor: 10000,
                note: Some("Prva smena".into()),
            },
        )
        .expect("first shift should open");

        let error = open_shift_for_user(
            &state,
            session.user.id,
            OpenShiftRequest {
                opening_cash_minor: 10000,
                note: None,
            },
        )
        .expect_err("second open shift should fail");

        assert_eq!(error.code, "validation_error");

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn closing_shift_stores_counted_cash_and_closed_status() {
        let db_path = test_database_path("closing_shift_stores_counted_cash_and_closed_status");
        let db = Db::new(&db_path).expect("database should initialize");
        let state = AppState::new(db);
        let session = login_user(
            &state,
            LoginRequest {
                username: "admin".into(),
                credential: "1234".into(),
            },
        )
        .expect("admin should log in");
        let shift = open_shift_for_user(
            &state,
            session.user.id,
            OpenShiftRequest {
                opening_cash_minor: 50000,
                note: None,
            },
        )
        .expect("shift should open");

        let closed = close_shift_for_user(
            &state,
            session.user.id,
            CloseShiftRequest {
                shift_id: shift.id,
                counted_cash_minor: 50000,
                note: Some("Bez razlike".into()),
            },
        )
        .expect("shift should close");

        assert_eq!(closed.status, "closed");
        assert_eq!(closed.counted_cash_minor, Some(50000));
        assert_eq!(closed.difference_minor, Some(0));

        let _ = std::fs::remove_file(db_path);
    }
}
