//! Small layout helpers shared by the views.

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::{
    AnyElement, App, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};

/// A labelled form field.
pub fn field(label: impl Into<SharedString>, input: impl IntoElement, cx: &App) -> AnyElement {
    v_flex()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(13.))
                .text_color(cx.theme().muted_foreground)
                .child(label.into()),
        )
        .child(input)
        .into_any_element()
}

/// A heading for a group of settings.
pub fn section(title: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .pt(px(8.))
        .pb(px(2.))
        .text_size(px(13.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(title.into())
        .into_any_element()
}

/// Secondary explanatory text.
pub fn hint(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
        .into_any_element()
}

/// An inline error message (renders nothing when `None`).
pub fn error(text: Option<&SharedString>, cx: &App) -> AnyElement {
    match text {
        Some(t) => div()
            .text_size(px(13.))
            .text_color(cx.theme().danger)
            .child(t.clone())
            .into_any_element(),
        None => div().into_any_element(),
    }
}

/// Relative time like "5 min ago" for list items.
pub fn relative_time(millis: i64) -> String {
    let now = cardfile_core::now_millis();
    let secs = (now - millis).max(0) / 1000;
    use cardfile_i18n::t_args;
    match secs {
        0..=59 => cardfile_i18n::t("time.just_now"),
        60..=3599 => t_args("time.minutes_ago", &[("n", &(secs / 60).to_string())]),
        3600..=86_399 => t_args("time.hours_ago", &[("n", &(secs / 3600).to_string())]),
        86_400..=604_799 => t_args("time.days_ago", &[("n", &(secs / 86_400).to_string())]),
        _ => format_date(millis),
    }
}

/// `YYYY-MM-DD HH:MM` in local time.
pub fn format_date(millis: i64) -> String {
    jiff::Timestamp::from_millisecond(millis)
        .map(|ts| {
            ts.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}
