//! Request preparation witnesses.

use super::super::request::extract_account_id;
use serde::Deserialize;

/// One credential and its surviving account claim.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountCase {
    /// Complete encoded credential.
    token: String,
    /// Account or an extraction failure.
    account: Option<String>,
}

#[test]
fn maestro_response_sessions_extract_account_claim() {
    let rows: Vec<AccountCase> =
        serde_json::from_str(include_str!("fixtures/account.json")).unwrap();
    for row in rows {
        match row.account {
            Some(account) => assert_eq!(extract_account_id(&row.token).unwrap(), account),
            None => assert_eq!(
                extract_account_id(&row.token).unwrap_err().message,
                "Failed to extract accountId from token"
            ),
        }
    }
}
