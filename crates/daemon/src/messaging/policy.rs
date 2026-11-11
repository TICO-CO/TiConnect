use crate::storage::PeerPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundDecision {
    Drop,
    Store {
        notify: bool,
        delivered_receipt: bool,
    },
}

pub fn inbound(policy: PeerPolicy, dnd: bool, receipts_enabled: bool) -> InboundDecision {
    match policy {
        PeerPolicy::Ignore => InboundDecision::Drop,
        PeerPolicy::Mute => InboundDecision::Store {
            notify: false,
            delivered_receipt: false,
        },
        PeerPolicy::Accept => InboundDecision::Store {
            notify: !dnd,
            delivered_receipt: receipts_enabled,
        },
    }
}

pub fn read_receipt(policy: PeerPolicy, receipts_enabled: bool) -> bool {
    receipts_enabled && matches!(policy, PeerPolicy::Accept | PeerPolicy::Mute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_drops_and_mute_stays_silent() {
        assert_eq!(
            inbound(PeerPolicy::Ignore, false, true),
            InboundDecision::Drop
        );
        assert_eq!(
            inbound(PeerPolicy::Mute, false, true),
            InboundDecision::Store {
                notify: false,
                delivered_receipt: false
            }
        );
    }

    #[test]
    fn dnd_suppresses_only_notification() {
        assert_eq!(
            inbound(PeerPolicy::Accept, true, true),
            InboundDecision::Store {
                notify: false,
                delivered_receipt: true
            }
        );
    }

    #[test]
    fn read_receipts_follow_the_global_toggle() {
        assert!(!read_receipt(PeerPolicy::Accept, false));
        assert!(read_receipt(PeerPolicy::Mute, true));
        assert!(!read_receipt(PeerPolicy::Ignore, true));
    }
}
