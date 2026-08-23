//! Identity guards for callbacks from replaceable Wayland protocol objects.

pub(super) fn is_current<T: PartialEq>(current: Option<T>, callback: T) -> bool {
    current == Some(callback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_unlocked_after_new_constraint_lock_is_rejected() {
        assert!(!is_current(Some(2_u64), 1));
        assert!(is_current(Some(2_u64), 2));
        assert!(!is_current(None, 2_u64));
    }

    #[test]
    fn relative_motion_from_replaced_proxy_is_rejected() {
        let old_relative_pointer = 41_u64;
        let current_relative_pointer = 42_u64;
        assert!(!is_current(
            Some(current_relative_pointer),
            old_relative_pointer
        ));
        assert!(is_current(
            Some(current_relative_pointer),
            current_relative_pointer
        ));
    }
}
