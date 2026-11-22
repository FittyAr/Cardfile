//! Shared pieces of the card protection (lock mode) selectors used by the card
//! header, the settings dialog and the first-run wizard.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{App, Window};
use notechest_core::LockMode;
use notechest_i18n::t;

/// The protections a card can have besides [`LockMode::None`], in display order.
pub const PROTECTIONS: [LockMode; 3] = [LockMode::ReadOnly, LockMode::Private, LockMode::Encrypted];

/// The user-visible name of a lock mode.
pub fn lock_label(mode: LockMode) -> String {
    t(&format!("lock.{}", mode.as_str()))
}

/// The user-visible explanation of a lock mode.
pub fn lock_description(mode: LockMode) -> String {
    t(&format!("lock.{}_desc", mode.as_str()))
}

/// The modes a selector offers: always [`LockMode::None`], then every enabled
/// mode in canonical order. `Encrypted` is left out unless `include_encrypted`
/// (a default mode cannot be `Encrypted`: it needs a password per card).
pub fn lock_choices(enabled: &[LockMode], include_encrypted: bool) -> Vec<LockMode> {
    LockMode::ALL
        .into_iter()
        .filter(|m| {
            *m == LockMode::None
                || (enabled.contains(m) && (include_encrypted || *m != LockMode::Encrypted))
        })
        .collect()
}

/// Fill a dropdown `menu` with one checked-when-current item per choice.
pub fn lock_menu(
    mut menu: PopupMenu,
    choices: &[LockMode],
    current: LockMode,
    on_pick: impl Fn(LockMode, &mut Window, &mut App) + Clone + 'static,
) -> PopupMenu {
    for &m in choices {
        let on_pick = on_pick.clone();
        menu = menu.item(
            PopupMenuItem::new(lock_label(m))
                .checked(m == current)
                .on_click(move |_, window, cx| on_pick(m, window, cx)),
        );
    }
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_always_offered_first() {
        assert_eq!(lock_choices(&[], true), vec![LockMode::None]);
        assert_eq!(lock_choices(&[], false), vec![LockMode::None]);
        assert_eq!(lock_choices(&[LockMode::None], true), vec![LockMode::None]);
    }

    #[test]
    fn choices_follow_canonical_order_not_enabled_order() {
        let enabled = [LockMode::Encrypted, LockMode::Private, LockMode::ReadOnly];
        assert_eq!(lock_choices(&enabled, true), LockMode::ALL.to_vec());
    }

    #[test]
    fn encrypted_is_dropped_when_not_allowed() {
        let enabled = [LockMode::Encrypted, LockMode::ReadOnly];
        assert_eq!(
            lock_choices(&enabled, false),
            vec![LockMode::None, LockMode::ReadOnly]
        );
        assert_eq!(
            lock_choices(&enabled, true),
            vec![LockMode::None, LockMode::ReadOnly, LockMode::Encrypted]
        );
    }

    #[test]
    fn duplicates_in_enabled_do_not_duplicate_choices() {
        let enabled = [LockMode::Private, LockMode::Private];
        assert_eq!(
            lock_choices(&enabled, true),
            vec![LockMode::None, LockMode::Private]
        );
    }

    #[test]
    fn protections_cover_every_mode_but_none() {
        assert_eq!(PROTECTIONS.as_slice(), &LockMode::ALL[1..]);
    }
}
