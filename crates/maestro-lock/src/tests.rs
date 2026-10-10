//! Controlled acquisition decisions.

use super::*;
use std::fs::TryLockError;
use std::time::Duration;

#[test]
fn retry_returns_success_at_each_boundary() {
    for success_at in [1, 2, 9, 10] {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = retry_contended(
            || {
                attempts += 1;
                if attempts == success_at {
                    Ok(())
                } else {
                    Err(TryLockError::WouldBlock)
                }
            },
            |delay| waits.push(delay),
        );
        assert!(result.is_ok());
        assert_eq!(attempts, success_at);
        assert_eq!(waits, vec![Duration::from_millis(20); success_at - 1]);
    }
}

#[test]
fn retry_exhausts_on_tenth_contention() {
    let mut attempts = 0;
    let mut waits = Vec::new();
    let result = retry_contended(
        || {
            attempts += 1;
            if attempts == 11 {
                Ok(())
            } else {
                Err(TryLockError::WouldBlock)
            }
        },
        |delay| waits.push(delay),
    );
    assert!(matches!(result, Err(TryLockError::WouldBlock)));
    assert_eq!(attempts, 10);
    assert_eq!(waits, vec![Duration::from_millis(20); 9]);
}

#[test]
fn retry_preserves_non_contention_failure() {
    for failure_at in [1, 4, 10] {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let payload = std::sync::Arc::new(17);
        let result = retry_contended(
            || {
                attempts += 1;
                if attempts == failure_at {
                    Err(TryLockError::Error(std::io::Error::other(Payload(
                        payload.clone(),
                    ))))
                } else {
                    Err(TryLockError::WouldBlock)
                }
            },
            |delay| waits.push(delay),
        );
        let Err(TryLockError::Error(error)) = result else {
            panic!("original error required")
        };
        let retained = error.get_ref().unwrap().downcast_ref::<Payload>().unwrap();
        assert!(std::sync::Arc::ptr_eq(&payload, &retained.0));
        assert_eq!(attempts, failure_at);
        assert_eq!(waits, vec![Duration::from_millis(20); failure_at - 1]);
    }
}

/// Identifiable native error payload.
#[derive(Debug)]
struct Payload(std::sync::Arc<u32>);

impl std::fmt::Display for Payload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("controlled failure")
    }
}
impl std::error::Error for Payload {}
