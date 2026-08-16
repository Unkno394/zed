use std::sync::Arc;

use fs::Fs;
use gpui::{
    Global, ObjectFit, ReadGlobal, ScrollHandle, SharedString, UpdateGlobal, img, prelude::*,
};
use settings::{Settings, SettingsStore, WindowOpacity, update_settings_file};
use theme::{ActiveTheme, SystemAppearance, Theme, ThemeRegistry};
use theme_settings::{ThemeName, ThemeSelection, ThemeSettings};
use ui::{h_flex, prelude::*, v_flex};

use crate::NumberFieldType;
use crate::SettingsWindow;
use crate::components::NumberField;

/// Snapshot of the theme settings from just before a hover-preview session started, so we
/// can restore them exactly when the pointer leaves (previews are never written to disk).
struct ThemeBrowserPreview(ThemeSettings);

impl Global for ThemeBrowserPreview {}

fn begin_preview(theme: &Theme, cx: &mut App) {
    let original = if let Some(preview) = cx.try_global::<ThemeBrowserPreview>() {
        preview.0.clone()
    } else {
        let original = SettingsStore::global(cx).get::<ThemeSettings>(None).clone();
        cx.set_global(ThemeBrowserPreview(original.clone()));
        original
    };

    let mut previewed = original;
    previewed.theme = ThemeSelection::Static(ThemeName(theme.name.clone().into()));

    SettingsStore::update_global(cx, |store, _| {
        store.override_global(previewed);
    });
}

fn end_preview(cx: &mut App) {
    if !cx.has_global::<ThemeBrowserPreview>() {
        return;
    }
    let ThemeBrowserPreview(original) = cx.remove_global::<ThemeBrowserPreview>();
    SettingsStore::update_global(cx, |store, _| {
        store.override_global(original);
    });
}

pub(crate) fn render_theme_browser_page(
    _settings_window: &SettingsWindow,
    scroll_handle: &ScrollHandle,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let registry = ThemeRegistry::global(cx);
    let active_theme_name = cx.theme().name.clone();

    let mut themes: Vec<Arc<Theme>> = registry
        .list()
        .into_iter()
        .filter_map(|meta| registry.get(&meta.name).ok())
        .collect();
    themes.sort_by(|a, b| a.name.cmp(&b.name));

    let mut gif_themes = Vec::new();
    let mut image_themes = Vec::new();
    let mut plain_themes = Vec::new();

    for theme in themes {
        match &theme.wallpaper {
            Some(path) if path.to_lowercase().ends_with(".gif") => gif_themes.push(theme),
            Some(_) => image_themes.push(theme),
            None => plain_themes.push(theme),
        }
    }

    let current_opacity =
        WindowOpacity(ThemeSettings::get_global(cx).window_opacity.unwrap_or(1.0));

    v_flex()
        .id("theme-browser-page")
        .min_w_0()
        .size_full()
        .pt_2p5()
        .px_8()
        .pb_16()
        .gap_6()
        .overflow_y_scroll()
        .track_scroll(scroll_handle)
        .on_mouse_up_out(gpui::MouseButton::Left, |_, _, cx| end_preview(cx))
        .child(render_opacity_control(current_opacity, window, cx))
        .child(
            Label::new("Hover a theme to preview it, click to keep it.")
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
        .child(render_theme_group(
            "Wallpaper (image)",
            image_themes,
            &active_theme_name,
        ))
        .child(render_theme_group(
            "Wallpaper (animated)",
            gif_themes,
            &active_theme_name,
        ))
        .child(render_theme_group(
            "No wallpaper (plain colors)",
            plain_themes,
            &active_theme_name,
        ))
        .into_any_element()
}

fn render_opacity_control(
    current_opacity: WindowOpacity,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    v_flex()
        .gap_1()
        .pb_2()
        .child(Label::new("Window Opacity").size(LabelSize::Default))
        .child(
            Label::new(
                "Applies to the editor, panels, tabs, and status bar of whichever theme is selected below.",
            )
            .size(LabelSize::Small)
            .color(Color::Muted),
        )
        .child(
            NumberField::new(
                "theme-browser-window-opacity",
                current_opacity,
                window,
                cx,
            )
            .min(WindowOpacity::min_value())
            .max(WindowOpacity::max_value())
            .on_change(|value, _window, cx| {
                let value = *value;
                update_settings_file(<dyn Fs>::global(cx), cx, move |settings_content, _| {
                    settings_content.theme.window_opacity = Some(value);
                });
            }),
        )
        .into_any_element()
}

fn render_theme_group(
    title: &'static str,
    themes: Vec<Arc<Theme>>,
    active_theme_name: &SharedString,
) -> AnyElement {
    if themes.is_empty() {
        return div().into_any_element();
    }

    v_flex()
        .gap_2()
        .child(
            Label::new(title)
                .size(LabelSize::Default)
                .color(Color::Muted),
        )
        .child(
            h_flex().flex_wrap().gap_3().children(
                themes
                    .into_iter()
                    .map(|theme| render_theme_tile(theme, active_theme_name)),
            ),
        )
        .into_any_element()
}

fn render_theme_tile(theme: Arc<Theme>, active_theme_name: &SharedString) -> AnyElement {
    let is_selected = theme.name == *active_theme_name;
    let theme_name = theme.name.clone();
    let theme_appearance = theme.appearance;
    let wallpaper = theme.wallpaper.clone();
    let swatch_color = theme.colors().editor_background;
    let border_color = theme.colors().border_focused;
    let border_transparent = theme.colors().border_transparent;
    let theme_for_hover = theme.clone();

    v_flex()
        .id(SharedString::from(format!("theme-tile-{}", theme_name)))
        .w(px(140.))
        .gap_1()
        .cursor_pointer()
        .child(
            div()
                .w_full()
                .h(px(84.))
                .rounded_md()
                .overflow_hidden()
                .border_2()
                .border_color(if is_selected {
                    border_color
                } else {
                    border_transparent
                })
                .bg(swatch_color)
                .when_some(wallpaper, |this, wallpaper| {
                    this.child(img(wallpaper).size_full().object_fit(ObjectFit::Cover))
                }),
        )
        .child(
            Label::new(theme_name.clone())
                .size(LabelSize::Small)
                .color(if is_selected {
                    Color::Accent
                } else {
                    Color::Default
                }),
        )
        .on_hover(move |hovered, _window, cx| {
            if *hovered {
                begin_preview(&theme_for_hover, cx);
            } else {
                end_preview(cx);
            }
        })
        .on_click(move |_event, _window, cx| {
            end_preview(cx);
            let fs = <dyn Fs>::global(cx);
            let theme_name: Arc<str> = theme_name.as_ref().into();
            let system_appearance = SystemAppearance::global(cx).0;
            update_settings_file(fs, cx, move |settings, _| {
                theme_settings::set_theme(
                    settings,
                    theme_name,
                    theme_appearance,
                    system_appearance,
                );
            });
        })
        .into_any_element()
}
