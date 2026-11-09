//! Registration table of the controlled guest bus: order, identity groups and snapshots.
use super::bus_host::Bank;

/// The registrations a bank holds, as channel and callback identity pairs.
fn pairs(entries: &[(&str, u32)]) -> Vec<(String, u32)> {
    entries
        .iter()
        .map(|(channel, id)| ((*channel).to_owned(), *id))
        .collect()
}

#[test]
fn removal_takes_every_registration_of_the_identity() -> Result<(), String> {
    let mut bank = Bank::default();
    let first = bank.on("a", 1)?;
    bank.on("b", 1)?;
    bank.on("a", 2)?;
    bank.unsubscribe(&first)?;
    assert_eq!(bank.listeners(), pairs(&[("a", 2)]));
    assert_eq!(bank.removals, [1]);
    bank.unsubscribe(&first)?;
    assert_eq!(bank.removals, [1, 1]);
    assert_eq!(bank.listeners(), pairs(&[("a", 2)]));
    Ok(())
}

#[test]
fn emission_captures_its_channel_in_registration_order_and_sees_later_edits() -> Result<(), String>
{
    let mut bank = Bank::default();
    let first = bank.on("a", 1)?;
    bank.on("b", 2)?;
    bank.on("a", 3)?;
    bank.on("a", 1)?;
    assert_eq!(bank.emit("a", "null")?, [1, 3, 1]);
    bank.unsubscribe(&first)?;
    bank.on("a", 4)?;
    assert_eq!(bank.emit("a", "null")?, [3, 4]);
    assert_eq!(bank.emit("c", "null")?, Vec::<u32>::new());
    assert_eq!(bank.emissions.len(), 3);
    Ok(())
}

#[test]
fn take_first_removes_one_registration_of_the_channel() -> Result<(), String> {
    let mut bank = Bank::default();
    bank.on("a", 1)?;
    bank.on("b", 1)?;
    bank.on("a", 1)?;
    assert_eq!(bank.take_first("a"), Some(1));
    assert_eq!(bank.listeners(), pairs(&[("b", 1), ("a", 1)]));
    assert_eq!(bank.take_first("c"), None);
    assert!(bank.removals.is_empty());
    Ok(())
}

#[test]
fn rejections_leave_the_table_unchanged() -> Result<(), String> {
    let mut bank = Bank::default();
    let kept = bank.on("a", 1)?;
    bank.registration_error = Some("no registration".into());
    assert_eq!(bank.on("a", 2).err(), Some("no registration".into()));
    bank.removal_error = Some("no removal".into());
    assert_eq!(bank.unsubscribe(&kept), Err("no removal".into()));
    bank.emission_error = Some("no emission".into());
    assert_eq!(bank.emit("a", "null"), Err("no emission".into()));
    assert_eq!(bank.listeners(), pairs(&[("a", 1)]));
    assert!(bank.removals.is_empty() && bank.emissions.is_empty());
    Ok(())
}

#[test]
fn resource_destruction_is_counted_apart_from_removal() -> Result<(), String> {
    let mut bank = Bank::default();
    let subscription = bank.on("a", 1)?;
    bank.unsubscribe(&subscription)?;
    let drops = std::sync::Arc::clone(&bank.subscription_drops);
    assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(subscription);
    assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(bank.removals, [1]);
    Ok(())
}
