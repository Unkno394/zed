use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use editor::EditorSettings;
use fs::{CopyOptions, Fs};
use futures::StreamExt as _;
use gpui::{
    Global, ObjectFit, PathPromptOptions, ReadGlobal, ScrollHandle, SharedString, UpdateGlobal,
    img, prelude::*,
};
use settings::{Settings, SettingsStore, WindowOpacity, update_settings_file};
use theme::{ActiveTheme, SystemAppearance, Theme, ThemeRegistry};
use theme_settings::{ThemeName, ThemeSelection, ThemeSettings};
use ui::{
    ContextMenu, DropdownMenu, DropdownStyle, IconPosition, SwitchField, ToggleState, h_flex,
    prelude::*, v_flex,
};

use crate::NumberFieldType;
use crate::SettingsWindow;
use crate::components::{NumberField, SettingsInputField};

/// Snapshot of the theme settings from just before a hover-preview session started, so we
/// can restore them exactly when the pointer leaves (previews are never written to disk).
struct ThemeBrowserPreview(ThemeSettings);

impl Global for ThemeBrowserPreview {}

#[derive(Default)]
struct CustomThemeDraft {
    name: String,
    palette_theme_name: Option<String>,
    status: Option<String>,
}

impl Global for CustomThemeDraft {}

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
    if !cx.has_global::<CustomThemeDraft>() {
        cx.set_global(CustomThemeDraft::default());
    }
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
    let wallpaper_settings = &EditorSettings::get_global(cx).wallpaper;
    let wallpaper_enabled = wallpaper_settings.enabled;
    let wallpaper_pinned = wallpaper_settings.pinned;

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
        .child(render_wallpaper_toggles(
            wallpaper_enabled,
            wallpaper_pinned,
            active_theme_name.clone(),
        ))
        .child(render_custom_theme_creator(
            active_theme_name.clone(),
            &plain_themes,
            cx.entity().downgrade(),
            window,
            cx,
        ))
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

fn render_wallpaper_toggles(
    wallpaper_enabled: bool,
    wallpaper_pinned: bool,
    active_theme_name: SharedString,
) -> AnyElement {
    v_flex()
        .gap_2()
        .child(
            SwitchField::new(
                "theme-browser-wallpaper-enabled",
                Some("Enable Wallpaper"),
                Some("Master switch. Off hides any wallpaper, on any theme.".into()),
                wallpaper_enabled,
                move |state, _window, cx| {
                    let enabled = *state == ToggleState::Selected;
                    update_settings_file(<dyn Fs>::global(cx), cx, move |settings_content, _| {
                        settings_content.editor.wallpaper_enabled = Some(enabled);
                    });
                },
            )
            .tab_index(0),
        )
        .child(
            SwitchField::new(
                "theme-browser-wallpaper-pinned",
                Some("Pin One Wallpaper Across Themes"),
                Some(
                    "On: locks in the wallpaper of whichever theme is active right now, so that \
                     image keeps showing even as you switch to other themes below. Off (default): \
                     each theme shows its own built-in wallpaper — a different image per theme, \
                     as before."
                        .into(),
                ),
                wallpaper_pinned,
                move |state, _window, cx| {
                    let pinned = *state == ToggleState::Selected;
                    let active_theme_name = active_theme_name.clone();
                    update_settings_file(<dyn Fs>::global(cx), cx, move |settings_content, _| {
                        settings_content.editor.wallpaper_pinned = Some(pinned);
                        if pinned {
                            settings_content.editor.wallpaper_pinned_theme =
                                Some(active_theme_name.to_string());
                        }
                    });
                },
            )
            .tab_index(1),
        )
        .into_any_element()
}

fn render_custom_theme_creator(
    active_theme_name: SharedString,
    plain_themes: &[Arc<Theme>],
    settings_window: gpui::WeakEntity<SettingsWindow>,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let draft = cx.global::<CustomThemeDraft>();
    let draft_name = draft.name.clone();
    let palette_theme_name = plain_themes
        .iter()
        .find(|theme| draft.palette_theme_name.as_deref() == Some(theme.name.as_ref()))
        .or_else(|| {
            plain_themes
                .iter()
                .find(|theme| theme.name == active_theme_name)
        })
        .or_else(|| plain_themes.first())
        .map(|theme| theme.name.to_string())
        .unwrap_or_default();
    let status = draft.status.clone();
    let name_settings_window = settings_window.clone();
    let palette_themes = plain_themes.to_vec();
    let selected_palette = palette_theme_name.clone();
    let palette_settings_window = settings_window.clone();
    let current_theme_name = active_theme_name.to_string();
    let palette_menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
        for theme in &palette_themes {
            let theme_name = theme.name.to_string();
            let is_selected = theme_name == selected_palette;
            let settings_window = palette_settings_window.clone();
            let current_theme_name = current_theme_name.clone();
            menu = menu.toggleable_entry(
                theme.name.clone(),
                is_selected,
                IconPosition::Start,
                None,
                move |_, cx| {
                    let mut draft = cx.remove_global::<CustomThemeDraft>();
                    draft.palette_theme_name = Some(theme_name.clone());
                    draft.status = None;
                    cx.set_global(draft);
                    let file_system = <dyn Fs>::global(cx);
                    let registry = ThemeRegistry::global(cx);
                    let system_appearance = SystemAppearance::global(cx).0;
                    let settings_window = settings_window.clone();
                    let selected_palette = theme_name.clone();
                    let current_theme_name = current_theme_name.clone();
                    cx.spawn(async move |cx| {
                        let result = apply_palette_to_custom_theme(
                            &file_system,
                            &registry,
                            &current_theme_name,
                            &selected_palette,
                        )
                        .await;
                        cx.update(|cx| {
                            let status = match result {
                                Ok(Some(created)) => {
                                    match theme_settings::load_user_theme(&registry, created.json.as_bytes()) {
                                        Ok(()) => {
                                            let theme_name: Arc<str> = created.name.clone().into();
                                            update_settings_file(file_system.clone(), cx, move |settings, _| {
                                                theme_settings::set_theme(settings, theme_name, created.appearance, system_appearance);
                                            });
                                            theme_settings::reload_theme(cx);
                                            format!("Applied {selected_palette} colors to the current wallpaper theme.")
                                        }
                                        Err(error) => format!("Could not load updated theme: {error:#}"),
                                    }
                                }
                                Ok(None) => format!("Selected {selected_palette} for the next uploaded wallpaper."),
                                Err(error) => format!("Could not apply palette: {error:#}"),
                            };
                            let mut draft = cx.remove_global::<CustomThemeDraft>();
                            draft.status = Some(status);
                            cx.set_global(draft);
                            if let Err(error) = settings_window.update(cx, |_, cx| cx.notify()) {
                                log::debug!("theme creator window closed: {error}");
                            }
                        });
                    }).detach();
                },
            );
        }
        menu
    });

    v_flex()
        .gap_2()
        .child(Label::new("Create a wallpaper theme").size(LabelSize::Default))
        .child(
            Label::new(format!(
                "Choose colors from a theme without wallpaper, then upload an image or GIF."
            ))
            .size(LabelSize::Small)
            .color(Color::Muted),
        )
        .child(
            DropdownMenu::new("custom-wallpaper-palette", palette_theme_name.clone(), palette_menu)
                .style(DropdownStyle::Outlined)
                .full_width(true)
                .aria_label("Color palette for the new wallpaper theme"),
        )
        .child(
            SettingsInputField::new("custom-wallpaper-theme-name")
                .with_initial_text(draft_name)
                .with_placeholder("Theme name (optional; press the checkmark to apply)")
                .display_confirm_button()
                .on_confirm(move |name, _window, cx| {
                    let mut draft = cx.remove_global::<CustomThemeDraft>();
                    draft.name = name.unwrap_or_default().trim().to_string();
                    cx.set_global(draft);
                    if let Err(error) = name_settings_window.update(cx, |_, cx| cx.notify()) {
                        log::debug!("theme creator window closed: {error}");
                    }
                }),
        )
        .child(
            Button::new("create-custom-wallpaper-theme", "Choose image or GIF")
                .style(ButtonStyle::Subtle)
                .on_click(move |_, _, cx| {
                    let selected_theme = palette_theme_name.clone();
                    let chosen_name = cx.global::<CustomThemeDraft>().name.clone();
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some("Choose a PNG, JPEG, WebP, or animated GIF".into()),
                    });
                    let file_system = <dyn Fs>::global(cx);
                    let registry = ThemeRegistry::global(cx);
                    let settings_window = settings_window.clone();
                    let system_appearance = SystemAppearance::global(cx).0;

                    cx.spawn(async move |cx| {
                        let result = async {
                            let paths = receiver.await.context("wallpaper picker closed")??;
                            let Some(path) = paths.and_then(|paths| paths.into_iter().next())
                            else {
                                return Ok(None);
                            };
                            create_custom_theme(
                                &file_system,
                                &registry,
                                &selected_theme,
                                &chosen_name,
                                &path,
                            )
                            .await
                            .map(Some)
                        }
                        .await;

                        cx.update(|cx| {
                            let status = match result {
                                Ok(None) => return,
                                Ok(Some(created)) => {
                                    match theme_settings::load_user_theme(
                                        &registry,
                                        created.json.as_bytes(),
                                    ) {
                                        Ok(()) => {
                                            theme_settings::reload_theme(cx);
                                            let theme_name: Arc<str> =
                                                created.name.clone().into();
                                            let appearance = created.appearance;
                                            update_settings_file(
                                                file_system.clone(),
                                                cx,
                                                move |settings, _| {
                                                    theme_settings::set_theme(
                                                        settings,
                                                        theme_name,
                                                        appearance,
                                                        system_appearance,
                                                    );
                                                    settings.editor.wallpaper_enabled = Some(true);
                                                    settings.editor.wallpaper_pinned = Some(false);
                                                },
                                            );
                                            format!(
                                                "Saved {} to {}. Edit this JSON to customize colors; Zed reloads it automatically.",
                                                created.name,
                                                created.path.display()
                                            )
                                        }
                                        Err(error) => format!("Could not load saved theme: {error:#}"),
                                    }
                                }
                                Err(error) => format!("Could not create theme: {error:#}"),
                            };
                            let mut draft = cx.remove_global::<CustomThemeDraft>();
                            draft.status = Some(status);
                            cx.set_global(draft);
                            if let Err(error) = settings_window.update(cx, |_, cx| cx.notify()) {
                                log::debug!("theme creator window closed: {error}");
                            }
                        });
                    })
                    .detach();
                }),
        )
        .when_some(status, |this, status| {
            this.child(Label::new(status).size(LabelSize::Small))
        })
        .into_any_element()
}

struct CreatedCustomTheme {
    name: String,
    path: PathBuf,
    json: String,
    appearance: theme::Appearance,
}

async fn apply_palette_to_custom_theme(
    file_system: &Arc<dyn Fs>,
    registry: &Arc<ThemeRegistry>,
    current_theme_name: &str,
    palette_theme_name: &str,
) -> anyhow::Result<Option<CreatedCustomTheme>> {
    let themes_dir = paths::themes_dir();
    if !file_system.is_dir(themes_dir).await {
        return Ok(None);
    }

    let mut files = file_system.read_dir(themes_dir).await?;
    while let Some(file) = files.next().await {
        let file = file?;
        let is_custom_wallpaper = file
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("custom-wallpaper-") && name.ends_with(".json"));
        if !is_custom_wallpaper {
            continue;
        }
        let bytes = file_system.load_bytes(&file).await?;
        let mut family = theme_settings::deserialize_user_theme(&bytes)?;
        let Some(theme) = family
            .themes
            .iter_mut()
            .find(|theme| theme.name == current_theme_name)
        else {
            continue;
        };
        let palette = find_theme_content(file_system, registry, palette_theme_name).await?;
        theme.appearance = palette.appearance;
        theme.style = palette.style;
        let json = serialize_custom_theme(&family)?;
        file_system.write(&file, json.as_bytes()).await?;
        let appearance = registry.get(palette_theme_name)?.appearance;
        return Ok(Some(CreatedCustomTheme {
            name: current_theme_name.to_string(),
            path: file,
            json,
            appearance,
        }));
    }
    Ok(None)
}

fn serialize_custom_theme(family: &theme_settings::ThemeFamilyContent) -> anyhow::Result<String> {
    let mut theme_json = serde_json::to_value(family)?;
    theme_json
        .as_object_mut()
        .context("Theme JSON must be an object")?
        .insert(
            "$schema".into(),
            "https://zed.dev/schema/themes/v0.2.0.json".into(),
        );
    Ok(format!("{}\n", serde_json::to_string_pretty(&theme_json)?))
}

async fn find_theme_content(
    file_system: &Arc<dyn Fs>,
    registry: &Arc<ThemeRegistry>,
    name: &str,
) -> anyhow::Result<theme_settings::ThemeContent> {
    if file_system.is_dir(paths::themes_dir()).await {
        let mut files = file_system.read_dir(paths::themes_dir()).await?;
        while let Some(file) = files.next().await {
            let file = file?;
            if file.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let bytes = match file_system.load_bytes(&file).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    log::warn!("Could not read theme at {}: {error:#}", file.display());
                    continue;
                }
            };
            match theme_settings::deserialize_user_theme(&bytes) {
                Ok(family) => {
                    if let Some(theme) = family.themes.into_iter().find(|theme| theme.name == name)
                    {
                        return Ok(theme);
                    }
                }
                Err(error) => log::warn!("Could not parse theme at {}: {error:#}", file.display()),
            }
        }
    }

    let installed_extensions = paths::extensions_dir().join("installed");
    if file_system.is_dir(&installed_extensions).await {
        let mut extensions = file_system.read_dir(&installed_extensions).await?;
        while let Some(extension) = extensions.next().await {
            let themes_dir = extension?.join("themes");
            if !file_system.is_dir(&themes_dir).await {
                continue;
            }
            let mut files = file_system.read_dir(&themes_dir).await?;
            while let Some(file) = files.next().await {
                let file = file?;
                if file.extension().and_then(|extension| extension.to_str()) != Some("json") {
                    continue;
                }
                let bytes = match file_system.load_bytes(&file).await {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        log::warn!("Could not read theme at {}: {error:#}", file.display());
                        continue;
                    }
                };
                match theme_settings::deserialize_user_theme(&bytes) {
                    Ok(family) => {
                        if let Some(theme) =
                            family.themes.into_iter().find(|theme| theme.name == name)
                        {
                            return Ok(theme);
                        }
                    }
                    Err(error) => {
                        log::warn!("Could not parse theme at {}: {error:#}", file.display())
                    }
                }
            }
        }
    }

    for path in registry.assets().list("themes/")? {
        if !path.ends_with(".json") {
            continue;
        }
        let Some(bytes) = registry.assets().load(&path)? else {
            continue;
        };
        match theme_settings::deserialize_user_theme(&bytes) {
            Ok(family) => {
                if let Some(theme) = family.themes.into_iter().find(|theme| theme.name == name) {
                    return Ok(theme);
                }
            }
            Err(error) => log::warn!("Could not parse bundled theme {path}: {error:#}"),
        }
    }

    anyhow::bail!("Colors for {name:?} could not be found in a theme JSON file")
}

async fn create_custom_theme(
    file_system: &Arc<dyn Fs>,
    registry: &Arc<ThemeRegistry>,
    base_theme_name: &str,
    chosen_name: &str,
    selected_image: &Path,
) -> anyhow::Result<CreatedCustomTheme> {
    let image = file_system.canonicalize(selected_image).await?;
    let extension = image
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .context("The selected image has no file extension")?;
    anyhow::ensure!(
        matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif"),
        "Choose a PNG, JPEG, WebP, or GIF image"
    );
    anyhow::ensure!(
        file_system.is_file(&image).await,
        "The selected image is not a file"
    );

    let base_theme = registry.get(base_theme_name)?;
    let base_content = find_theme_content(file_system, registry, base_theme_name).await?;
    let image_name = image
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Wallpaper");
    let requested_name = if chosen_name.trim().is_empty() {
        format!("{image_name} Wallpaper")
    } else {
        chosen_name.trim().to_string()
    };

    let themes_dir = paths::themes_dir();
    let wallpapers_dir = paths::config_dir().join("wallpapers");
    file_system.create_dir(themes_dir).await?;
    file_system.create_dir(&wallpapers_dir).await?;

    let preview_bytes = if extension == "gif" {
        let bytes = file_system.load_bytes(&image).await?;
        let first_frame = image::load_from_memory_with_format(&bytes, image::ImageFormat::Gif)
            .context("Could not decode the selected GIF")?;
        let mut preview = std::io::Cursor::new(Vec::new());
        first_frame
            .thumbnail(280, 158)
            .write_to(&mut preview, image::ImageFormat::Png)?;
        Some(preview.into_inner())
    } else {
        None
    };
    let previews_dir = wallpapers_dir.join("previews");
    if preview_bytes.is_some() {
        file_system.create_dir(&previews_dir).await?;
    }

    let (name, theme_path, wallpaper_path, preview_path) = {
        let mut available = None;
        for index in 1..=10_000 {
            let name = if index == 1 {
                requested_name.clone()
            } else {
                format!("{requested_name} ({index})")
            };
            let theme_path = themes_dir.join(format!("custom-wallpaper-{index}.json"));
            let wallpaper_path =
                wallpapers_dir.join(format!("custom-wallpaper-{index}.{extension}"));
            let preview_path = previews_dir.join(format!("custom-wallpaper-{index}.png"));
            if registry.get(&name).is_err()
                && file_system.metadata(&theme_path).await?.is_none()
                && file_system.metadata(&wallpaper_path).await?.is_none()
                && (preview_bytes.is_none() || file_system.metadata(&preview_path).await?.is_none())
            {
                available = Some((name, theme_path, wallpaper_path, preview_path));
                break;
            }
        }
        available.context("Too many custom wallpaper themes")?
    };

    let theme = theme_settings::ThemeContent {
        name: name.clone(),
        wallpaper: Some(wallpaper_path.to_string_lossy().into_owned()),
        wallpaper_scale: None,
        wallpaper_offset_x: None,
        wallpaper_offset_y: None,
        wallpaper_anchor: None,
        wallpaper_opacity: Some(0.25),
        ..base_content
    };
    let family = theme_settings::ThemeFamilyContent {
        name: format!("{name} Wallpapers"),
        author: "Local User".into(),
        themes: vec![theme],
    };
    let json = serialize_custom_theme(&family)?;

    file_system
        .copy_file(&image, &wallpaper_path, CopyOptions::default())
        .await
        .with_context(|| format!("copying image to {}", wallpaper_path.display()))?;
    if let Some(preview_bytes) = preview_bytes
        && let Err(error) = file_system.write(&preview_path, &preview_bytes).await
    {
        if let Err(cleanup_error) = file_system
            .remove_file(&wallpaper_path, fs::RemoveOptions::default())
            .await
        {
            log::warn!("Could not remove incomplete wallpaper copy: {cleanup_error:#}");
        }
        return Err(error)
            .with_context(|| format!("writing preview to {}", preview_path.display()));
    }
    if let Err(error) = file_system.write(&theme_path, json.as_bytes()).await {
        if let Err(cleanup_error) = file_system
            .remove_file(&wallpaper_path, fs::RemoveOptions::default())
            .await
        {
            log::warn!("Could not remove incomplete wallpaper copy: {cleanup_error:#}");
        }
        if extension == "gif"
            && let Err(cleanup_error) = file_system
                .remove_file(&preview_path, fs::RemoveOptions::default())
                .await
        {
            log::warn!("Could not remove incomplete wallpaper preview: {cleanup_error:#}");
        }
        return Err(error).with_context(|| format!("writing theme to {}", theme_path.display()));
    }

    Ok(CreatedCustomTheme {
        name,
        path: theme_path,
        json,
        appearance: base_theme.appearance,
    })
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
    let wallpaper = theme.wallpaper.clone().map(wallpaper_preview_path);
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
                    let image = if std::path::Path::new(wallpaper.as_ref()).is_absolute() {
                        img(std::path::Path::new(wallpaper.as_ref()))
                    } else {
                        img(wallpaper)
                    };
                    this.child(image.size_full().object_fit(ObjectFit::Cover))
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

fn wallpaper_preview_path(wallpaper: SharedString) -> SharedString {
    if let Some(name) = wallpaper
        .strip_prefix("images/wallpapers/custom/")
        .filter(|name| name.ends_with(".gif"))
    {
        return format!("images/wallpapers/custom/previews/{name}").into();
    }

    let file_path = Path::new(wallpaper.as_ref());
    let wallpapers_dir = paths::config_dir().join("wallpapers");
    if file_path.parent() == Some(wallpapers_dir.as_path())
        && file_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("gif")
        && let Some(stem) = file_path.file_stem().and_then(|stem| stem.to_str())
        && stem.starts_with("custom-wallpaper-")
    {
        return wallpapers_dir
            .join("previews")
            .join(format!("{stem}.png"))
            .to_string_lossy()
            .into_owned()
            .into();
    }

    wallpaper
}
