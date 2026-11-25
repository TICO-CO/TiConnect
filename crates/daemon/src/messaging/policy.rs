use crate::storage::UserPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundDecision {
    Drop,
    Store {
        notify: bool,
        delivered_receipt: bool,
    },
}

pub fn inbound(policy: UserPolicy, dnd: bool, receipts_enabled: bool) -> InboundDecision {
    match policy {
        UserPolicy::Ignore => InboundDecision::Drop,
        UserPolicy::Mute => InboundDecision::Store {
            notify: false,
            delivered_receipt: false,
        },
        UserPolicy::Accept => InboundDecision::Store {
            notify: !dnd,
            delivered_receipt: receipts_enabled,
        },
    }
}

pub fn read_receipt(policy: UserPolicy, receipts_enabled: bool) -> bool {
    receipts_enabled && matches!(policy, UserPolicy::Accept | UserPolicy::Mute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_drops_and_mute_stays_silent() {
        assert_eq!(
            inbound(UserPolicy::Ignore, false, true),
            InboundDecision::Drop
        );
        assert_eq!(
            inbound(UserPolicy::Mute, false, true),
            InboundDecision::Store {
                notify: false,
                delivered_receipt: false
            }
        );
    }

    #[test]
    fn dnd_suppresses_only_notification() {
        assert_eq!(
            inbound(UserPolicy::Accept, true, true),
            InboundDecision::Store {
                notify: false,
                delivered_receipt: true
            }
        );
    }

    #[test]
    fn read_receipts_follow_the_global_toggle() {
        assert!(!read_receipt(UserPolicy::Accept, false));
        assert!(read_receipt(UserPolicy::Mute, true));
        assert!(!read_receipt(UserPolicy::Ignore, true));
    }
}
