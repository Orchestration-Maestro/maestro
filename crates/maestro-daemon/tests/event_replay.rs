//! Shared ordered replay of retained daemon events.
#![cfg(test)]

use maestro_daemon::api::event_hub::EventHub;
use maestro_protocol::events::EventData;

#[test]
fn event_replay_keeps_sequence_capacity_and_shared_owners() {
    let hub = EventHub::default();
    let other = hub.clone();
    assert_eq!(hub.current_sequence(), 0);
    assert!(hub.events_after(0).is_empty());
    for index in 0..514 {
        let event = EventData::WorkspaceRenamed {
            workspace_id: "w1".into(),
            label: format!("label-{index}"),
        };
        if index % 2 == 0 {
            hub.push(event);
        } else {
            other.push(event);
        }
    }
    assert_eq!(hub.current_sequence(), 514);
    assert_eq!(other.current_sequence(), 514);
    let replay = other.events_after(0);
    assert_eq!(replay.len(), 512);
    for (index, (sequence, event)) in replay.iter().enumerate() {
        assert_eq!(*sequence, index as u64 + 3);
        assert_eq!(
            *event,
            EventData::WorkspaceRenamed {
                workspace_id: "w1".into(),
                label: format!("label-{}", index + 2),
            }
        );
    }
    assert_eq!(hub.events_after(512), replay[510..]);
    assert!(hub.events_after(514).is_empty());
    assert!(hub.events_after(u64::MAX).is_empty());
}
