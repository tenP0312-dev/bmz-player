use bmz_core::lane::KeyMode;
use bmz_render::scene::SelectRowKind;

use crate::config::app_config::{AppConfig, AudioBackend, AudioBufferSizeMode};
use crate::config::app_settings_registry::{AppSettingsEntryId, format_app_settings_value};
use crate::config::key_config::{
    KEY_BINDING_SLOTS, KEY_CONFIG_MODES, KeyBindingGroup, KeyBindingSlot, KeyBindingTarget,
    binding_row_label, format_play_binding, key_binding_targets_for_group,
    key_mode_binding_targets, key_mode_settings_path,
};
use crate::config::profile_config::ProfileConfig;
use crate::config::settings_registry::{SettingsEntryId, format_settings_value};
use crate::i18n::{AppLocale, Localizer};
use crate::screens::select_model::SelectItem;
use crate::ui::Lr2SkinTarget;

pub const CONFIG_ROOT_PATH: &str = "bmz-settings:";
const CONFIG_VOLUME_PATH: &str = "bmz-settings:volume";
const CONFIG_AUDIO_PATH: &str = "bmz-settings:audio";
const CONFIG_JUDGE_PATH: &str = "bmz-settings:judge";
const CONFIG_PLAY_PATH: &str = "bmz-settings:play";
const CONFIG_PLAY_SEVEN_TO_NINE_PATH: &str = "bmz-settings:play:seven-to-nine";
const CONFIG_PLAY_ASSIST_PATH: &str = "bmz-settings:play:assist";
const CONFIG_PLAY_ASSIST_NOTE_PATH: &str = "bmz-settings:play:assist:note";
const CONFIG_PLAY_ASSIST_JUDGE_PATH: &str = "bmz-settings:play:assist:judge";
const CONFIG_DISPLAY_PATH: &str = "bmz-settings:display";
const CONFIG_VIDEO_PATH: &str = "bmz-settings:video";
const CONFIG_INPUT_PATH: &str = "bmz-settings:input";
const CONFIG_SELECT_PATH: &str = "bmz-settings:select";
const CONFIG_REPLAY_PATH: &str = "bmz-settings:replay";
const CONFIG_UI_PATH: &str = "bmz-settings:ui";
pub const CONFIG_KEYS_PATH: &str = "bmz-settings:keys";
const CONFIG_KEYS_COMMON_PATH: &str = "bmz-settings:keys:common";
const CONFIG_KEYS_SELECT_PATH: &str = "bmz-settings:keys:select";
const CONFIG_KEYS_PLAY_PATH: &str = "bmz-settings:keys:play";
const CONFIG_KEYS_RESULT_PATH: &str = "bmz-settings:keys:result";
const CONFIG_PROFILE_PATH: &str = "bmz-settings:profile";
const CONFIG_SKIN_PATH: &str = "bmz-settings:skin";
const CONFIG_SKIN_SLOT_PREFIX: &str = "bmz-settings:skin:";
const CONFIG_LIBRARY_PATH: &str = "bmz-settings:library";
const CONFIG_LIBRARY_SONGS_PATH: &str = "bmz-settings:library:songs";
const CONFIG_LIBRARY_TABLES_PATH: &str = "bmz-settings:library:tables";
const CONFIG_LIBRARY_DOWNLOADS_PATH: &str = "bmz-settings:library:downloads";
const CONFIG_IR_PATH: &str = "bmz-settings:ir";
const CONFIG_SYSTEM_PATH: &str = "bmz-settings:system";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPath<'a> {
    Root,
    Volume,
    Audio,
    Judge,
    Play,
    PlaySevenToNine,
    PlayAssist,
    PlayAssistNote,
    PlayAssistJudge,
    Display,
    Video,
    Input,
    Select,
    Replay,
    Ui,
    KeysRoot,
    KeysCommon,
    KeysSelect,
    KeysPlay,
    KeysResult,
    KeysMode(KeyMode),
    Profile,
    SkinRoot,
    Skin(Lr2SkinTarget),
    Library,
    LibrarySongs,
    LibraryTables,
    LibraryDownloads,
    Ir,
    System,
    Unknown(&'a str),
}

pub fn parse_settings_path(path: &str) -> Option<SettingsPath<'_>> {
    let rest = path.strip_prefix(CONFIG_ROOT_PATH)?;
    match rest {
        "" => Some(SettingsPath::Root),
        "volume" => Some(SettingsPath::Volume),
        "audio" => Some(SettingsPath::Audio),
        "judge" => Some(SettingsPath::Judge),
        "play" => Some(SettingsPath::Play),
        "play:seven-to-nine" => Some(SettingsPath::PlaySevenToNine),
        "play:assist" => Some(SettingsPath::PlayAssist),
        "play:assist:note" => Some(SettingsPath::PlayAssistNote),
        "play:assist:judge" => Some(SettingsPath::PlayAssistJudge),
        "display" => Some(SettingsPath::Display),
        "video" => Some(SettingsPath::Video),
        "input" => Some(SettingsPath::Input),
        "select" => Some(SettingsPath::Select),
        "replay" => Some(SettingsPath::Replay),
        "ui" => Some(SettingsPath::Ui),
        "keys" => Some(SettingsPath::KeysRoot),
        "keys:common" => Some(SettingsPath::KeysCommon),
        "keys:select" => Some(SettingsPath::KeysSelect),
        "keys:play" => Some(SettingsPath::KeysPlay),
        "keys:result" => Some(SettingsPath::KeysResult),
        _ if let Some(mode_key) = rest.strip_prefix("keys:") => {
            KeyMode::from_play_map_key(mode_key).map(SettingsPath::KeysMode)
        }
        "profile" => Some(SettingsPath::Profile),
        "skin" => Some(SettingsPath::SkinRoot),
        _ if let Some(slot_key) = rest.strip_prefix("skin:") => {
            Lr2SkinTarget::from_key(slot_key).map(SettingsPath::Skin)
        }
        "library" => Some(SettingsPath::Library),
        "library:songs" => Some(SettingsPath::LibrarySongs),
        "library:tables" => Some(SettingsPath::LibraryTables),
        "library:downloads" => Some(SettingsPath::LibraryDownloads),
        "ir" => Some(SettingsPath::Ir),
        "system" => Some(SettingsPath::System),
        other => Some(SettingsPath::Unknown(other)),
    }
}

pub fn in_settings_stack(stack: &[String]) -> bool {
    stack.last().is_some_and(|path| path.starts_with(CONFIG_ROOT_PATH))
}

pub fn settings_breadcrumb(path: &str) -> String {
    settings_breadcrumb_for_locale(path, AppLocale::DEFAULT)
}

pub fn settings_breadcrumb_for_locale(path: &str, locale: AppLocale) -> String {
    let text = Localizer::new(locale);
    let root = text.text("settings-category-root");
    match parse_settings_path(path) {
        Some(SettingsPath::Root) | None => root,
        Some(SettingsPath::Volume) => breadcrumb(&root, &text.text("settings-category-volume")),
        Some(SettingsPath::Audio) => breadcrumb(&root, &text.text("settings-audio-title")),
        Some(SettingsPath::Judge) => breadcrumb(&root, &text.text("settings-category-judge")),
        Some(SettingsPath::Play) => breadcrumb(&root, &text.text("settings-category-play")),
        Some(SettingsPath::PlaySevenToNine) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-category-play"),
            text.text("settings-category-seven-to-nine")
        ),
        Some(SettingsPath::PlayAssist) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-category-play"),
            text.text("settings-category-assist")
        ),
        Some(SettingsPath::PlayAssistNote) => {
            assist_breadcrumb(&root, text, "settings-category-assist-note")
        }
        Some(SettingsPath::PlayAssistJudge) => {
            assist_breadcrumb(&root, text, "settings-category-assist-judge")
        }
        Some(SettingsPath::Display) => breadcrumb(&root, &text.text("settings-category-display")),
        Some(SettingsPath::Video) => breadcrumb(&root, &text.text("settings-video-title")),
        Some(SettingsPath::Input) => breadcrumb(&root, &text.text("settings-category-input")),
        Some(SettingsPath::Select) => breadcrumb(&root, &text.text("settings-category-select")),
        Some(SettingsPath::Replay) => breadcrumb(&root, &text.text("settings-category-replay")),
        Some(SettingsPath::Ui) => breadcrumb(&root, &text.text("settings-category-ui")),
        Some(SettingsPath::KeysRoot) => breadcrumb(&root, &text.text("settings-category-keys")),
        Some(SettingsPath::KeysCommon) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-category-keys"),
            text.text("settings-category-common")
        ),
        Some(SettingsPath::KeysSelect) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-category-keys"),
            text.text("settings-nav-select")
        ),
        Some(SettingsPath::KeysPlay) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-category-keys"),
            text.text("settings-nav-play")
        ),
        Some(SettingsPath::KeysResult) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-category-keys"),
            text.text("settings-nav-result")
        ),
        Some(SettingsPath::KeysMode(key_mode)) => {
            format!("{} > {} > {}", root, text.text("settings-category-keys"), key_mode.as_str())
        }
        Some(SettingsPath::Profile) => breadcrumb(&root, &text.text("settings-nav-profile")),
        Some(SettingsPath::SkinRoot) => breadcrumb(&root, &text.text("settings-nav-skin")),
        Some(SettingsPath::Skin(target)) => {
            format!("{} > {} > {}", root, text.text("settings-nav-skin"), target.scene_label(text))
        }
        Some(SettingsPath::Library) => breadcrumb(&root, &text.text("settings-nav-library")),
        Some(SettingsPath::LibrarySongs) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-nav-library"),
            text.text("settings-song-folders")
        ),
        Some(SettingsPath::LibraryTables) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-nav-library"),
            text.text("settings-tables-title")
        ),
        Some(SettingsPath::LibraryDownloads) => format!(
            "{} > {} > {}",
            root,
            text.text("settings-nav-library"),
            text.text("settings-downloads-title")
        ),
        Some(SettingsPath::Ir) => breadcrumb(&root, &text.text("profile-ir-title")),
        Some(SettingsPath::System) => breadcrumb(&root, &text.text("settings-category-system")),
        Some(SettingsPath::Unknown(_)) => root,
    }
}

fn breadcrumb(root: &str, section: &str) -> String {
    format!("{root} > {section}")
}

fn assist_breadcrumb(root: &str, text: Localizer, leaf_key: &str) -> String {
    format!(
        "{} > {} > {} > {}",
        root,
        text.text("settings-category-play"),
        text.text("settings-category-assist"),
        text.text(leaf_key)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigSelectRow {
    pub entry_id: SettingsEntryId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfigSelectRow {
    pub entry_id: AppSettingsEntryId,
}

impl AppConfigSelectRow {
    pub fn label(self) -> &'static str {
        self.entry_id.label()
    }

    pub fn value_text(self, config: &AppConfig, locale: AppLocale) -> String {
        format_app_settings_value(config, self.entry_id, locale)
    }

    pub fn description_text(self, profile: &ProfileConfig) -> String {
        Localizer::new(profile.ui.locale()).text(self.entry_id.description_key())
    }
}

impl ConfigSelectRow {
    pub fn label(self) -> &'static str {
        self.entry_id.label()
    }

    pub fn value_text(self, profile: &ProfileConfig) -> String {
        format_settings_value(profile, self.entry_id)
    }

    pub fn description_text(self, profile: &ProfileConfig) -> String {
        Localizer::new(profile.ui.locale()).text(self.entry_id.description_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBindingSelectRow {
    pub key_mode: KeyMode,
    pub target: KeyBindingTarget,
}

impl KeyBindingSelectRow {
    pub fn label(self) -> String {
        binding_row_label(self.key_mode, self.target)
    }

    pub fn value_text(self, profile: &ProfileConfig) -> String {
        format_play_binding(profile, self.key_mode, self.target)
    }

    pub fn description_text(self, profile: &ProfileConfig) -> String {
        Localizer::new(profile.ui.locale()).text("settings-key-binding-description")
    }
}

/// One value of a [`SettingsChoiceRow`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsChoice {
    /// The stored value (profile id, skin path, ...). Empty means "default" where
    /// the setting has one.
    pub key: String,
    pub label: String,
}

impl SettingsChoice {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self { key: key.into(), label: label.into() }
    }
}

/// What a [`SettingsChoiceRow`] changes. The values are listed when the folder
/// is built; the app applies the confirmed one through the same path as the BMZ menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsChoiceTarget {
    /// Switch to another existing profile.
    Profile,
    SkinPath(Lr2SkinTarget),
    /// Customize row of the slot's skin, in `lr2_skin_item_choices` order.
    SkinItem(Lr2SkinTarget, usize),
    SongRootEnabled(usize),
    SongRootRecursive(usize),
    TableEnabled(usize),
    /// 0 = 1P, 1 = 2P.
    GamepadSlot(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsChoiceRow {
    pub target: SettingsChoiceTarget,
    pub label: String,
    pub description: String,
    pub choices: Vec<SettingsChoice>,
    pub current: usize,
}

impl SettingsChoiceRow {
    pub fn value_text(&self) -> String {
        self.choices
            .get(self.current)
            .map_or_else(|| "-".to_string(), |choice| choice.label.clone())
    }

    /// Moves to the next / previous value, wrapping around.
    pub fn step(&mut self, direction: i32) -> bool {
        let len = self.choices.len();
        if len < 2 || direction == 0 {
            return false;
        }
        let next = (self.current as i64 + i64::from(direction.signum())).rem_euclid(len as i64);
        self.current = next as usize;
        true
    }
}

/// Which key bindings a "restore defaults" row resets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyDefaultsScope {
    Group(KeyBindingGroup),
    Mode(KeyMode),
}

/// A row that runs an operation when confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsActionKind {
    RescanSongs,
    FullRescanSongs,
    FetchEnabledTables,
    FetchTable(usize),
    GamepadAutoAssign,
    GamepadSwap,
    GamepadClearAssignment,
    RestoreKeyDefaults { scope: KeyDefaultsScope, slot: KeyBindingSlot },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsActionRow {
    pub kind: SettingsActionKind,
    pub label: String,
    pub description: String,
}

/// Data the settings folder needs besides the two configs. The app collects it
/// on the window thread before the list is built.
#[derive(Debug, Clone, Default)]
pub struct SettingsFolderExtras {
    /// Existing profiles as `(id, display name)`.
    pub profiles: Vec<(String, String)>,
    /// Connected controllers as `(stable id, label)`.
    pub gamepads: Vec<(String, String)>,
    /// Display names of difficulty tables by URL.
    pub table_names: Vec<(String, String)>,
    pub skin: Option<SkinFolderData>,
}

impl SettingsFolderExtras {
    pub const EMPTY: Self =
        Self { profiles: Vec::new(), gamepads: Vec::new(), table_names: Vec::new(), skin: None };
}

/// The skins and customize values of one skin slot.
#[derive(Debug, Clone)]
pub struct SkinFolderData {
    pub target: Lr2SkinTarget,
    pub skins: Vec<SettingsChoice>,
    pub current_skin: usize,
    pub items: Vec<(String, Vec<SettingsChoice>, usize)>,
}

/// The settings path whose folder needs skin data, if any.
pub fn settings_skin_target(path: &str) -> Option<Lr2SkinTarget> {
    match parse_settings_path(path)? {
        SettingsPath::Skin(target) => Some(target),
        _ => None,
    }
}

pub fn settings_root_item() -> SelectItem {
    settings_root_item_for_locale(AppLocale::DEFAULT)
}

pub fn settings_root_item_for_locale(locale: AppLocale) -> SelectItem {
    SelectItem::Folder {
        path: CONFIG_ROOT_PATH.to_string(),
        name: Localizer::new(locale).text("settings-category-root"),
        kind: SelectRowKind::SettingsRoot,
        summary: None,
    }
}

pub fn load_settings_items(path: &str) -> Vec<SelectItem> {
    load_settings_items_for_locale(path, AppLocale::DEFAULT)
}

pub fn load_settings_items_for_locale(path: &str, locale: AppLocale) -> Vec<SelectItem> {
    load_settings_items_for_config(
        path,
        locale,
        &AppConfig::default(),
        &ProfileConfig::new_default("default", "Default", 0),
        &SettingsFolderExtras::EMPTY,
    )
}

pub fn load_settings_items_for_config(
    path: &str,
    locale: AppLocale,
    app_config: &AppConfig,
    profile_config: &ProfileConfig,
    extras: &SettingsFolderExtras,
) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let settings_path = parse_settings_path(path);
    let mut items = match settings_path {
        Some(SettingsPath::Root) => vec![
            SelectItem::Folder {
                path: CONFIG_VOLUME_PATH.to_string(),
                name: text.text("settings-category-volume"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_AUDIO_PATH.to_string(),
                name: text.text("settings-audio-title"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_JUDGE_PATH.to_string(),
                name: text.text("settings-category-judge"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_PLAY_PATH.to_string(),
                name: text.text("settings-category-play"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_DISPLAY_PATH.to_string(),
                name: text.text("settings-category-display"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_VIDEO_PATH.to_string(),
                name: text.text("settings-video-title"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_INPUT_PATH.to_string(),
                name: text.text("settings-category-input"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_SELECT_PATH.to_string(),
                name: text.text("settings-category-select"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_REPLAY_PATH.to_string(),
                name: text.text("settings-category-replay"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_UI_PATH.to_string(),
                name: text.text("settings-category-ui"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            SelectItem::Folder {
                path: CONFIG_KEYS_PATH.to_string(),
                name: text.text("settings-category-keys"),
                kind: SelectRowKind::SettingsFolder,
                summary: None,
            },
            settings_folder(CONFIG_PROFILE_PATH, text.text("settings-nav-profile")),
            settings_folder(CONFIG_SKIN_PATH, text.text("settings-nav-skin")),
            settings_folder(CONFIG_LIBRARY_PATH, text.text("settings-nav-library")),
            settings_folder(CONFIG_IR_PATH, text.text("profile-ir-title")),
            settings_folder(CONFIG_SYSTEM_PATH, text.text("settings-category-system")),
            SelectItem::AdvancedSettings,
        ],
        Some(SettingsPath::Volume) => config_items(SettingsEntryId::VOLUME_ENTRIES),
        Some(SettingsPath::Audio) => audio_items(app_config),
        Some(SettingsPath::Judge) => config_items(SettingsEntryId::JUDGE_ENTRIES),
        Some(SettingsPath::Play) => play_items(locale),
        Some(SettingsPath::PlaySevenToNine) => config_items(SettingsEntryId::SEVEN_TO_NINE_ENTRIES),
        Some(SettingsPath::PlayAssist) => assist_items(locale),
        Some(SettingsPath::PlayAssistNote) => config_items(SettingsEntryId::ASSIST_NOTE_ENTRIES),
        Some(SettingsPath::PlayAssistJudge) => config_items(SettingsEntryId::ASSIST_JUDGE_ENTRIES),
        Some(SettingsPath::Display) => display_items(profile_config),
        Some(SettingsPath::Video) => app_config_items(AppSettingsEntryId::VIDEO_ENTRIES),
        Some(SettingsPath::Input) => input_items(locale, app_config, extras),
        Some(SettingsPath::Select) => {
            let mut items = config_items(SettingsEntryId::SELECT_ENTRIES);
            items.extend(app_config_items(AppSettingsEntryId::SELECT_ENTRIES));
            items
        }
        Some(SettingsPath::Replay) => config_items(SettingsEntryId::REPLAY_ENTRIES),
        Some(SettingsPath::Ui) => config_items(SettingsEntryId::UI_ENTRIES),
        Some(SettingsPath::KeysRoot) => key_mode_folder_items(locale),
        Some(SettingsPath::KeysCommon) => key_group_binding_items(KeyBindingGroup::Common, locale),
        Some(SettingsPath::KeysSelect) => key_group_binding_items(KeyBindingGroup::Select, locale),
        Some(SettingsPath::KeysPlay) => key_group_binding_items(KeyBindingGroup::Play, locale),
        Some(SettingsPath::KeysResult) => key_group_binding_items(KeyBindingGroup::Result, locale),
        Some(SettingsPath::KeysMode(key_mode)) => key_binding_items(key_mode, locale),
        Some(SettingsPath::Profile) => profile_items(locale, profile_config, extras),
        Some(SettingsPath::SkinRoot) => Lr2SkinTarget::ALL
            .into_iter()
            .map(|target| {
                settings_folder(
                    &format!("{CONFIG_SKIN_SLOT_PREFIX}{}", target.key()),
                    target.scene_label(text),
                )
            })
            .collect(),
        Some(SettingsPath::Skin(target)) => skin_items(locale, target, extras),
        Some(SettingsPath::Library) => vec![
            settings_folder(CONFIG_LIBRARY_SONGS_PATH, text.text("settings-song-folders")),
            settings_folder(CONFIG_LIBRARY_TABLES_PATH, text.text("settings-tables-title")),
            settings_folder(CONFIG_LIBRARY_DOWNLOADS_PATH, text.text("settings-downloads-title")),
        ],
        Some(SettingsPath::LibrarySongs) => song_root_items(locale, app_config),
        Some(SettingsPath::LibraryTables) => table_items(locale, app_config, extras),
        Some(SettingsPath::LibraryDownloads) => {
            app_config_items(AppSettingsEntryId::DOWNLOADS_ENTRIES)
        }
        Some(SettingsPath::Ir) => config_items(SettingsEntryId::IR_ENTRIES),
        Some(SettingsPath::System) => app_config_items(AppSettingsEntryId::SYSTEM_ENTRIES),
        Some(SettingsPath::Unknown(_)) | None => Vec::new(),
    };
    if !items.is_empty() {
        let action = if settings_path == Some(SettingsPath::Root) {
            SelectItem::SettingsClose
        } else {
            SelectItem::SettingsBack
        };
        items.insert(0, action);
    }
    items
}

fn settings_folder(path: &str, name: String) -> SelectItem {
    SelectItem::Folder {
        path: path.to_string(),
        name,
        kind: SelectRowKind::SettingsFolder,
        summary: None,
    }
}

fn audio_items(app_config: &AppConfig) -> Vec<SelectItem> {
    let mut entries = vec![AppSettingsEntryId::AudioBackend];
    if app_config.audio.backend == AudioBackend::Wasapi {
        entries.push(AppSettingsEntryId::AudioOutputMode);
    }
    entries.push(AppSettingsEntryId::AudioSampleRate);
    entries.push(AppSettingsEntryId::AudioBufferMode);
    if app_config.audio.buffer_size_mode == AudioBufferSizeMode::Fixed {
        entries.push(AppSettingsEntryId::AudioBufferSize);
    }
    if app_config.audio.backend == AudioBackend::Asio {
        entries.push(AppSettingsEntryId::AudioAsioDriver);
        entries.push(AppSettingsEntryId::AudioOutputChannelPair);
    } else {
        entries.push(AppSettingsEntryId::AudioOutputDevice);
    }
    let mut items = app_config_items(&entries);
    items.push(SelectItem::ApplyAudioSettings);
    items
}

fn play_items(locale: AppLocale) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let mut items = Vec::new();
    for entry_id in SettingsEntryId::PLAY_ENTRIES {
        items.push(SelectItem::Config(ConfigSelectRow { entry_id: *entry_id }));
        if *entry_id == SettingsEntryId::KeyModeConversion {
            items.push(settings_folder(
                CONFIG_PLAY_SEVEN_TO_NINE_PATH,
                text.text("settings-category-seven-to-nine"),
            ));
            items.push(settings_folder(
                CONFIG_PLAY_ASSIST_PATH,
                text.text("settings-category-assist"),
            ));
        }
    }
    items
}

fn assist_items(locale: AppLocale) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let mut items = config_items(SettingsEntryId::ASSIST_ENTRIES);
    items.push(settings_folder(
        CONFIG_PLAY_ASSIST_NOTE_PATH,
        text.text("settings-category-assist-note"),
    ));
    items.push(settings_folder(
        CONFIG_PLAY_ASSIST_JUDGE_PATH,
        text.text("settings-category-assist-judge"),
    ));
    items
}

fn key_mode_folder_items(locale: AppLocale) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    [
        (CONFIG_KEYS_COMMON_PATH, "settings-category-common"),
        (CONFIG_KEYS_SELECT_PATH, "settings-nav-select"),
        (CONFIG_KEYS_PLAY_PATH, "settings-nav-play"),
        (CONFIG_KEYS_RESULT_PATH, "settings-nav-result"),
    ]
    .into_iter()
    .map(|(path, label)| SelectItem::Folder {
        path: path.to_string(),
        name: text.text(label),
        kind: SelectRowKind::SettingsFolder,
        summary: None,
    })
    .chain(KEY_CONFIG_MODES.iter().copied().map(|key_mode| SelectItem::Folder {
        path: key_mode_settings_path(CONFIG_KEYS_PATH, key_mode),
        name: key_mode.as_str().to_string(),
        kind: SelectRowKind::SettingsFolder,
        summary: None,
    }))
    .collect()
}

fn key_group_binding_items(group: KeyBindingGroup, locale: AppLocale) -> Vec<SelectItem> {
    let mut items: Vec<SelectItem> = KEY_BINDING_SLOTS
        .iter()
        .copied()
        .flat_map(|slot| {
            key_binding_targets_for_group(group, slot).into_iter().map(|target| {
                SelectItem::KeyBinding(KeyBindingSelectRow { key_mode: KeyMode::K7, target })
            })
        })
        .collect();
    items.extend(key_defaults_items(KeyDefaultsScope::Group(group), locale));
    items
}

/// One "restore defaults" row per binding slot, as in the BMZ menu.
fn key_defaults_items(scope: KeyDefaultsScope, locale: AppLocale) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    KEY_BINDING_SLOTS
        .iter()
        .copied()
        .filter(|slot| match scope {
            KeyDefaultsScope::Group(group) => {
                !key_binding_targets_for_group(group, *slot).is_empty()
            }
            KeyDefaultsScope::Mode(key_mode) => {
                !key_mode_binding_targets(key_mode, *slot).is_empty()
            }
        })
        .map(|slot| {
            SelectItem::SettingsAction(SettingsActionRow {
                kind: SettingsActionKind::RestoreKeyDefaults { scope, slot },
                label: format!("{} ({})", text.text("profile-key-config-defaults"), slot.suffix()),
                description: text.text("settings-entry-description-restore-key-defaults"),
            })
        })
        .collect()
}

fn key_binding_items(key_mode: KeyMode, locale: AppLocale) -> Vec<SelectItem> {
    let hispeed_rows = (key_mode == KeyMode::K8)
        .then_some(SettingsEntryId::HISPEED_8K_ENTRIES)
        .into_iter()
        .flatten()
        .copied()
        .map(|entry_id| SelectItem::Config(ConfigSelectRow { entry_id }));
    let binding_rows = KEY_BINDING_SLOTS.iter().copied().flat_map(|slot| {
        key_mode_binding_targets(key_mode, slot)
            .into_iter()
            .map(move |target| SelectItem::KeyBinding(KeyBindingSelectRow { key_mode, target }))
    });
    hispeed_rows
        .chain(binding_rows)
        .chain(key_defaults_items(KeyDefaultsScope::Mode(key_mode), locale))
        .collect()
}

fn on_off_choices() -> Vec<SettingsChoice> {
    vec![SettingsChoice::new("off", "OFF"), SettingsChoice::new("on", "ON")]
}

fn choice_row(
    target: SettingsChoiceTarget,
    label: String,
    description: String,
    choices: Vec<SettingsChoice>,
    current: usize,
) -> SelectItem {
    SelectItem::SettingsChoice(SettingsChoiceRow { target, label, description, choices, current })
}

fn action_row(kind: SettingsActionKind, label: String, description: String) -> SelectItem {
    SelectItem::SettingsAction(SettingsActionRow { kind, label, description })
}

fn input_items(
    locale: AppLocale,
    app_config: &AppConfig,
    extras: &SettingsFolderExtras,
) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let mut items = config_items(SettingsEntryId::INPUT_ENTRIES);
    items.extend(app_config_items(AppSettingsEntryId::INPUT_ENTRIES));
    for (slot, label_key) in
        [(0usize, "settings-input-controller-1p"), (1usize, "settings-input-controller-2p")]
    {
        let mut choices = vec![SettingsChoice::new("", text.text("settings-input-auto-order"))];
        choices.extend(
            extras
                .gamepads
                .iter()
                .map(|(id, label)| SettingsChoice::new(id.clone(), label.clone())),
        );
        let configured = app_config.input.gamepad_slot_device_ids[slot].clone().unwrap_or_default();
        if !configured.is_empty() && !choices.iter().any(|choice| choice.key == configured) {
            // A disconnected device stays selectable so the row shows the saved value.
            let short: String = configured.chars().take(20).collect();
            choices.push(SettingsChoice::new(configured.clone(), format!("{short}...")));
        }
        let current = choices.iter().position(|choice| choice.key == configured).unwrap_or(0);
        items.push(choice_row(
            SettingsChoiceTarget::GamepadSlot(slot),
            text.text(label_key),
            text.text("settings-entry-description-gamepad-slot"),
            choices,
            current,
        ));
    }
    let help = text.text("settings-input-assignment-help");
    items.push(action_row(
        SettingsActionKind::GamepadAutoAssign,
        text.text("settings-input-auto-assign"),
        help.clone(),
    ));
    items.push(action_row(
        SettingsActionKind::GamepadSwap,
        text.text("settings-input-swap"),
        help.clone(),
    ));
    items.push(action_row(
        SettingsActionKind::GamepadClearAssignment,
        text.text("settings-input-clear-assignment"),
        help,
    ));
    items
}

fn profile_items(
    locale: AppLocale,
    profile: &ProfileConfig,
    extras: &SettingsFolderExtras,
) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let mut choices: Vec<SettingsChoice> = extras
        .profiles
        .iter()
        .map(|(id, name)| SettingsChoice::new(id.clone(), profile_choice_label(id, name)))
        .collect();
    if !choices.iter().any(|choice| choice.key == profile.id) {
        choices.insert(
            0,
            SettingsChoice::new(
                profile.id.clone(),
                profile_choice_label(&profile.id, &profile.display_name),
            ),
        );
    }
    let current = choices.iter().position(|choice| choice.key == profile.id).unwrap_or(0);
    vec![choice_row(
        SettingsChoiceTarget::Profile,
        "PROFILE".to_string(),
        text.text("settings-entry-description-profile-switch"),
        choices,
        current,
    )]
}

fn profile_choice_label(id: &str, display_name: &str) -> String {
    if display_name.is_empty() || display_name == id {
        id.to_string()
    } else {
        format!("{display_name} ({id})")
    }
}

fn skin_items(
    locale: AppLocale,
    target: Lr2SkinTarget,
    extras: &SettingsFolderExtras,
) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let Some(data) = extras.skin.as_ref().filter(|data| data.target == target) else {
        return Vec::new();
    };
    let mut items = vec![choice_row(
        SettingsChoiceTarget::SkinPath(target),
        "SKIN".to_string(),
        text.text("settings-entry-description-skin-path"),
        data.skins.clone(),
        data.current_skin,
    )];
    let option_description = text.text("settings-entry-description-skin-option");
    items.extend(data.items.iter().enumerate().map(|(index, (name, choices, current))| {
        choice_row(
            SettingsChoiceTarget::SkinItem(target, index),
            name.clone(),
            option_description.clone(),
            choices.clone(),
            *current,
        )
    }));
    items
}

fn song_root_items(locale: AppLocale, app_config: &AppConfig) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let mut items = Vec::new();
    for (index, root) in app_config.songs.roots.iter().enumerate() {
        items.push(choice_row(
            SettingsChoiceTarget::SongRootEnabled(index),
            root.path.clone(),
            text.text("settings-entry-description-song-root-enabled"),
            on_off_choices(),
            usize::from(root.enabled),
        ));
        items.push(choice_row(
            SettingsChoiceTarget::SongRootRecursive(index),
            format!("{} - {}", text.text("settings-song-recursive"), path_leaf(&root.path)),
            text.text("settings-entry-description-song-root-recursive"),
            on_off_choices(),
            usize::from(root.recursive),
        ));
    }
    items.push(action_row(
        SettingsActionKind::RescanSongs,
        text.text("settings-library-rescan"),
        text.text("settings-song-scan-help"),
    ));
    items.push(action_row(
        SettingsActionKind::FullRescanSongs,
        text.text("settings-library-reparse-all"),
        text.text("settings-library-reparse-all-help"),
    ));
    items.extend(app_config_items(AppSettingsEntryId::SCAN_ENTRIES));
    items
}

fn path_leaf(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|leaf| !leaf.is_empty())
        .unwrap_or(path)
}

fn table_items(
    locale: AppLocale,
    app_config: &AppConfig,
    extras: &SettingsFolderExtras,
) -> Vec<SelectItem> {
    let text = Localizer::new(locale);
    let help = text.text("settings-tables-help");
    let mut items = vec![action_row(
        SettingsActionKind::FetchEnabledTables,
        text.text("settings-tables-fetch-enabled"),
        help.clone(),
    )];
    for (index, source) in app_config.tables.sources.iter().enumerate() {
        let name = extras
            .table_names
            .iter()
            .find(|(url, _)| *url == source.url)
            .map(|(_, name)| name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| source.url.clone());
        items.push(choice_row(
            SettingsChoiceTarget::TableEnabled(index),
            name.clone(),
            text.text("settings-entry-description-table-enabled"),
            on_off_choices(),
            usize::from(source.enabled),
        ));
        items.push(action_row(
            SettingsActionKind::FetchTable(index),
            format!("{} - {name}", text.text("common-fetch")),
            help.clone(),
        ));
    }
    items.extend(app_config_items(AppSettingsEntryId::TABLES_ENTRIES));
    items
}

fn config_items(entries: &'static [SettingsEntryId]) -> Vec<SelectItem> {
    entries
        .iter()
        .copied()
        .map(|entry_id| SelectItem::Config(ConfigSelectRow { entry_id }))
        .collect()
}

fn display_items(profile: &ProfileConfig) -> Vec<SelectItem> {
    let preset = profile.lane.hispeed_config();
    SettingsEntryId::DISPLAY_ENTRIES
        .iter()
        .copied()
        .filter(|entry_id| entry_id.visible_for_hispeed_config(preset))
        .map(|entry_id| SelectItem::Config(ConfigSelectRow { entry_id }))
        .collect()
}

fn app_config_items(entries: &[AppSettingsEntryId]) -> Vec<SelectItem> {
    entries
        .iter()
        .copied()
        .map(|entry_id| SelectItem::AppConfig(AppConfigSelectRow { entry_id }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::key_config::{COMMON_ACTIONS, KeyBindingSlot, ScratchDirection};
    use crate::config::profile_config::{InputActionConfig, LaneConfig};

    fn key_defaults_rows(items: &[SelectItem]) -> usize {
        let count = items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    SelectItem::SettingsAction(SettingsActionRow {
                        kind: SettingsActionKind::RestoreKeyDefaults { .. },
                        ..
                    })
                )
            })
            .count();
        assert!(count > 0, "every key folder has restore-defaults rows");
        count
    }

    #[test]
    fn parse_settings_paths() {
        assert_eq!(parse_settings_path(CONFIG_ROOT_PATH), Some(SettingsPath::Root));
        assert_eq!(parse_settings_path(CONFIG_VOLUME_PATH), Some(SettingsPath::Volume));
        assert_eq!(parse_settings_path(CONFIG_AUDIO_PATH), Some(SettingsPath::Audio));
        assert_eq!(parse_settings_path(CONFIG_JUDGE_PATH), Some(SettingsPath::Judge));
        assert_eq!(parse_settings_path(CONFIG_PLAY_PATH), Some(SettingsPath::Play));
        assert_eq!(
            parse_settings_path(CONFIG_PLAY_SEVEN_TO_NINE_PATH),
            Some(SettingsPath::PlaySevenToNine)
        );
        assert_eq!(parse_settings_path(CONFIG_PLAY_ASSIST_PATH), Some(SettingsPath::PlayAssist));
        assert_eq!(
            parse_settings_path(CONFIG_PLAY_ASSIST_NOTE_PATH),
            Some(SettingsPath::PlayAssistNote)
        );
        assert_eq!(
            parse_settings_path(CONFIG_PLAY_ASSIST_JUDGE_PATH),
            Some(SettingsPath::PlayAssistJudge)
        );
        assert_eq!(parse_settings_path(CONFIG_DISPLAY_PATH), Some(SettingsPath::Display));
        assert_eq!(parse_settings_path(CONFIG_VIDEO_PATH), Some(SettingsPath::Video));
        assert_eq!(parse_settings_path(CONFIG_INPUT_PATH), Some(SettingsPath::Input));
        assert_eq!(parse_settings_path(CONFIG_SELECT_PATH), Some(SettingsPath::Select));
        assert_eq!(parse_settings_path(CONFIG_REPLAY_PATH), Some(SettingsPath::Replay));
        assert_eq!(parse_settings_path(CONFIG_UI_PATH), Some(SettingsPath::Ui));
        assert_eq!(parse_settings_path(CONFIG_KEYS_PATH), Some(SettingsPath::KeysRoot));
        assert_eq!(parse_settings_path(CONFIG_KEYS_COMMON_PATH), Some(SettingsPath::KeysCommon));
        assert_eq!(
            parse_settings_path("bmz-settings:keys:7k"),
            Some(SettingsPath::KeysMode(KeyMode::K7))
        );
        assert!(parse_settings_path("/songs").is_none());
    }

    #[test]
    fn settings_root_lists_categories() {
        let items = load_settings_items(CONFIG_ROOT_PATH);
        assert_eq!(items.len(), 18);
        assert!(matches!(items.first(), Some(SelectItem::SettingsClose)));
        assert!(matches!(
            settings_root_item(),
            SelectItem::Folder { kind: SelectRowKind::SettingsRoot, .. }
        ));
        assert!(matches!(items.last(), Some(SelectItem::AdvancedSettings)));
        assert!(matches!(
            &items[1],
            SelectItem::Folder { name, .. } if name == "音量"
        ));

        let english = load_settings_items_for_locale(CONFIG_ROOT_PATH, AppLocale::En);
        assert!(matches!(
            &english[1],
            SelectItem::Folder { name, .. } if name == "Volume"
        ));
        assert_eq!(
            settings_breadcrumb_for_locale(CONFIG_KEYS_COMMON_PATH, AppLocale::Ko),
            "설정 > 키 설정 > 공통"
        );
    }

    #[test]
    fn settings_audio_lists_backend_specific_entries_and_apply_action() {
        let mut app_config = AppConfig::default();
        app_config.audio.backend = AudioBackend::Wasapi;
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let wasapi = load_settings_items_for_config(
            CONFIG_AUDIO_PATH,
            AppLocale::Ja,
            &app_config,
            &profile,
            &SettingsFolderExtras::EMPTY,
        );
        assert!(wasapi.iter().any(|item| matches!(
            item,
            SelectItem::AppConfig(row)
                if row.entry_id == AppSettingsEntryId::AudioOutputMode
        )));
        assert!(matches!(wasapi.last(), Some(SelectItem::ApplyAudioSettings)));

        app_config.audio.backend = AudioBackend::Asio;
        let asio = load_settings_items_for_config(
            CONFIG_AUDIO_PATH,
            AppLocale::Ja,
            &app_config,
            &profile,
            &SettingsFolderExtras::EMPTY,
        );
        assert!(asio.iter().any(|item| matches!(
            item,
            SelectItem::AppConfig(row)
                if row.entry_id == AppSettingsEntryId::AudioAsioDriver
        )));
        assert!(asio.iter().any(|item| matches!(
            item,
            SelectItem::AppConfig(row)
                if row.entry_id == AppSettingsEntryId::AudioOutputChannelPair
        )));
        assert!(!asio.iter().any(|item| matches!(
            item,
            SelectItem::AppConfig(row)
                if row.entry_id == AppSettingsEntryId::AudioOutputDevice
        )));
    }

    #[test]
    fn settings_video_lists_every_video_entry() {
        let items = load_settings_items(CONFIG_VIDEO_PATH);
        assert_eq!(items.len(), AppSettingsEntryId::VIDEO_ENTRIES.len() + 1);
        for entry_id in AppSettingsEntryId::VIDEO_ENTRIES {
            assert!(items.iter().any(|item| matches!(
                item,
                SelectItem::AppConfig(row) if row.entry_id == *entry_id
            )));
        }
    }

    #[test]
    fn settings_select_lists_random_select_and_random_mix_options() {
        let items = load_settings_items(CONFIG_SELECT_PATH);
        assert_eq!(
            items.len(),
            SettingsEntryId::SELECT_ENTRIES.len() + AppSettingsEntryId::SELECT_ENTRIES.len() + 1
        );
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::SelectRandomSelect
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::RandomMixTargetLevel
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::RandomMixStages
        )));
    }

    #[test]
    fn settings_volume_lists_entries() {
        let items = load_settings_items(CONFIG_VOLUME_PATH);
        assert_eq!(items.len(), SettingsEntryId::VOLUME_ENTRIES.len() + 1);
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(
            matches!(&items[1], SelectItem::Config(row) if row.entry_id == SettingsEntryId::NormalizeChartVolume)
        );
        assert!(
            matches!(&items[2], SelectItem::Config(row) if row.entry_id == SettingsEntryId::NormalizeSystemBgmVolume)
        );
        assert!(
            matches!(&items[3], SelectItem::Config(row) if row.entry_id == SettingsEntryId::MasterVolume)
        );
    }

    #[test]
    fn settings_keys_lists_key_mode_folders() {
        let items = load_settings_items(CONFIG_KEYS_PATH);
        assert_eq!(items.len(), KEY_CONFIG_MODES.len() + 5);
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(matches!(
            &items[1],
            SelectItem::Folder { name, path, .. }
                if name == "共通" && path == CONFIG_KEYS_COMMON_PATH
        ));
        assert!(matches!(
            &items[2],
            SelectItem::Folder { name, path, .. }
                if name == "選曲" && path == CONFIG_KEYS_SELECT_PATH
        ));
        assert!(matches!(
            &items[3],
            SelectItem::Folder { name, path, .. }
                if name == "プレイ" && path == CONFIG_KEYS_PLAY_PATH
        ));
        assert!(matches!(
            &items[4],
            SelectItem::Folder { name, path, .. }
                if name == "リザルト" && path == CONFIG_KEYS_RESULT_PATH
        ));
        assert!(matches!(
            &items[5],
            SelectItem::Folder { name, path, .. }
                if name == "4K" && path == "bmz-settings:keys:4k"
        ));
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            let expected_path = format!("bmz-settings:keys:{}", key_mode.play_map_key());
            assert!(items.iter().any(|item| matches!(
                item,
                SelectItem::Folder { name, path, .. }
                    if name == key_mode.as_str() && path == &expected_path
            )));
        }
    }

    #[test]
    fn settings_keys_common_lists_configurable_actions() {
        let items = load_settings_items(CONFIG_KEYS_COMMON_PATH);
        assert_eq!(
            items.len(),
            COMMON_ACTIONS.len() * KEY_BINDING_SLOTS.len() + key_defaults_rows(&items) + 1
        );
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(matches!(
            &items[1],
            SelectItem::KeyBinding(row)
                if row.target == KeyBindingTarget::Action {
                    action: InputActionConfig::E1,
                    slot: KeyBindingSlot::KeyboardPrimary,
                }
        ));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::KeyBinding(row)
                if row.target == KeyBindingTarget::Action {
                    action: InputActionConfig::E4,
                    slot: KeyBindingSlot::Controller,
                }
        )));
        let select_items = load_settings_items(CONFIG_KEYS_SELECT_PATH);
        assert!(select_items.iter().any(|item| matches!(
            item,
            SelectItem::KeyBinding(row)
                if row.target == KeyBindingTarget::Action {
                    action: InputActionConfig::Screenshot,
                    slot: KeyBindingSlot::KeyboardPrimary,
                }
        )));
        assert!(!items.iter().any(|item| matches!(
            item,
            SelectItem::KeyBinding(row)
                if matches!(
                    row.target,
                    KeyBindingTarget::Action {
                        action: InputActionConfig::SelectEnter | InputActionConfig::SelectOptionBga,
                        ..
                    }
                )
        )));
    }

    #[test]
    fn settings_keys_7k_lists_lanes() {
        let items = load_settings_items("bmz-settings:keys:7k");
        assert_eq!(items.len(), 9 * KEY_BINDING_SLOTS.len() + key_defaults_rows(&items) + 1);
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(matches!(
            &items[1],
            SelectItem::KeyBinding(row)
                if row.key_mode == KeyMode::K7
                    && row.target == KeyBindingTarget::Scratch {
                        lane: LaneConfig::Scratch,
                        direction: ScratchDirection::Up,
                        slot: KeyBindingSlot::KeyboardPrimary,
                    }
        ));
        assert!(matches!(
            &items[2],
            SelectItem::KeyBinding(row)
                if row.key_mode == KeyMode::K7
                    && row.target == KeyBindingTarget::Scratch {
                        lane: LaneConfig::Scratch,
                        direction: ScratchDirection::Down,
                        slot: KeyBindingSlot::KeyboardPrimary,
                    }
        ));
        assert!(matches!(
            &items[10],
            SelectItem::KeyBinding(row)
                if row.key_mode == KeyMode::K7
                    && row.target == KeyBindingTarget::Scratch {
                        lane: LaneConfig::Scratch,
                        direction: ScratchDirection::Up,
                        slot: KeyBindingSlot::KeyboardSecondary,
                    }
        ));
        assert!(matches!(
            &items[19],
            SelectItem::KeyBinding(row)
                if row.key_mode == KeyMode::K7
                    && row.target == KeyBindingTarget::Scratch {
                        lane: LaneConfig::Scratch,
                        direction: ScratchDirection::Up,
                        slot: KeyBindingSlot::Controller,
                    }
        ));
    }

    #[test]
    fn settings_keys_14k_lists_lanes() {
        let items = load_settings_items("bmz-settings:keys:14k");
        assert_eq!(items.len(), 18 * KEY_BINDING_SLOTS.len() + key_defaults_rows(&items) + 1);
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::KeyBinding(row)
                if row.target == KeyBindingTarget::Key {
                    lane: LaneConfig::Key1,
                    slot: KeyBindingSlot::Controller1P,
                }
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::KeyBinding(row)
                if row.target == KeyBindingTarget::Key {
                    lane: LaneConfig::Key8,
                    slot: KeyBindingSlot::Controller2P,
                }
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::KeyBinding(row)
                if row.target == KeyBindingTarget::Scratch {
                    lane: LaneConfig::Scratch2,
                    direction: ScratchDirection::Up,
                    slot: KeyBindingSlot::Controller2P,
                }
        )));
    }

    #[test]
    fn settings_keys_extension_modes_list_lanes() {
        for (key_mode, rows_per_slot) in
            [(KeyMode::K4, 4), (KeyMode::K6, 6), (KeyMode::K8, 8), (KeyMode::K9, 9)]
        {
            let items =
                load_settings_items(&format!("bmz-settings:keys:{}", key_mode.play_map_key()));
            let hispeed_rows =
                if key_mode == KeyMode::K8 { SettingsEntryId::HISPEED_8K_ENTRIES.len() } else { 0 };
            assert_eq!(
                items.len(),
                rows_per_slot * KEY_BINDING_SLOTS.len()
                    + hispeed_rows
                    + key_defaults_rows(&items)
                    + 1
            );
            assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
            assert!(items.iter().any(|item| matches!(
                item,
                SelectItem::KeyBinding(row) if row.key_mode == key_mode
            )));
        }
    }

    #[test]
    fn settings_keys_8k_lists_each_hispeed_direction() {
        let items = load_settings_items("bmz-settings:keys:8k");
        let entries: Vec<_> = items
            .iter()
            .filter_map(|item| match item {
                SelectItem::Config(row)
                    if SettingsEntryId::HISPEED_8K_ENTRIES.contains(&row.entry_id) =>
                {
                    Some(row.entry_id)
                }
                _ => None,
            })
            .collect();

        assert_eq!(entries, SettingsEntryId::HISPEED_8K_ENTRIES);
    }

    #[test]
    fn settings_play_lists_gauge_entry() {
        let items = load_settings_items(CONFIG_PLAY_PATH);
        assert_eq!(items.len(), SettingsEntryId::PLAY_ENTRIES.len() + 3);
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::Gauge
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::RuleMode
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::LnModePolicy
        )));
        assert!(!items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::Assist
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::SessionMode
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Folder { path, .. } if path == CONFIG_PLAY_SEVEN_TO_NINE_PATH
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Folder { path, .. } if path == CONFIG_PLAY_ASSIST_PATH
        )));
    }

    #[test]
    fn settings_play_subfolders_list_conversion_and_assist_fields() {
        let seven_to_nine = load_settings_items(CONFIG_PLAY_SEVEN_TO_NINE_PATH);
        assert_eq!(seven_to_nine.len(), SettingsEntryId::SEVEN_TO_NINE_ENTRIES.len() + 1);
        assert!(seven_to_nine.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::SevenToNineRuleMode
        )));

        let assist = load_settings_items(CONFIG_PLAY_ASSIST_PATH);
        assert_eq!(assist.len(), SettingsEntryId::ASSIST_ENTRIES.len() + 3);
        assert!(assist.iter().any(|item| matches!(
            item,
            SelectItem::Folder { path, .. } if path == CONFIG_PLAY_ASSIST_NOTE_PATH
        )));
        assert!(assist.iter().any(|item| matches!(
            item,
            SelectItem::Folder { path, .. } if path == CONFIG_PLAY_ASSIST_JUDGE_PATH
        )));

        let note = load_settings_items(CONFIG_PLAY_ASSIST_NOTE_PATH);
        assert_eq!(note.len(), SettingsEntryId::ASSIST_NOTE_ENTRIES.len() + 1);
        let judge = load_settings_items(CONFIG_PLAY_ASSIST_JUDGE_PATH);
        assert_eq!(judge.len(), SettingsEntryId::ASSIST_JUDGE_ENTRIES.len() + 1);
    }

    #[test]
    fn settings_display_starts_with_hispeed_config_and_hides_unused_hispeed_entries() {
        let app_config = AppConfig::default();
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        let common = [
            SettingsEntryId::SuddenEnabled,
            SettingsEntryId::Sudden,
            SettingsEntryId::LiftEnabled,
            SettingsEntryId::Lift,
            SettingsEntryId::HiddenEnabled,
            SettingsEntryId::Hidden,
            SettingsEntryId::Constant,
            SettingsEntryId::ConstantFadeMs,
            SettingsEntryId::HispeedTransitionMs,
            SettingsEntryId::InvertE2KeyGreenNumber,
        ];
        let cases = [
            (
                crate::config::profile_config::HispeedConfigPreset::Normal,
                vec![SettingsEntryId::HispeedMode, SettingsEntryId::NormalHispeedLevel],
            ),
            (
                crate::config::profile_config::HispeedConfigPreset::Classic,
                vec![
                    SettingsEntryId::HispeedMode,
                    SettingsEntryId::Hispeed,
                    SettingsEntryId::ClassicHispeedStep,
                ],
            ),
            (
                crate::config::profile_config::HispeedConfigPreset::Lr2,
                vec![
                    SettingsEntryId::HispeedMode,
                    SettingsEntryId::Hispeed,
                    SettingsEntryId::ClassicHispeedStep,
                ],
            ),
            (
                crate::config::profile_config::HispeedConfigPreset::Floating,
                vec![
                    SettingsEntryId::HispeedMode,
                    SettingsEntryId::FloatingHispeedStep,
                    SettingsEntryId::TargetGreenNumber,
                    SettingsEntryId::NoteDisplayDurationMs,
                    SettingsEntryId::HispeedAutoAdjust,
                ],
            ),
            (
                crate::config::profile_config::HispeedConfigPreset::NormalFloating,
                vec![
                    SettingsEntryId::HispeedMode,
                    SettingsEntryId::NormalHispeedLevel,
                    SettingsEntryId::FloatingHispeedStep,
                    SettingsEntryId::TargetGreenNumber,
                    SettingsEntryId::NoteDisplayDurationMs,
                    SettingsEntryId::HispeedAutoAdjust,
                ],
            ),
            (
                crate::config::profile_config::HispeedConfigPreset::ClassicFloating,
                vec![
                    SettingsEntryId::HispeedMode,
                    SettingsEntryId::Hispeed,
                    SettingsEntryId::ClassicHispeedStep,
                    SettingsEntryId::FloatingHispeedStep,
                    SettingsEntryId::TargetGreenNumber,
                    SettingsEntryId::NoteDisplayDurationMs,
                    SettingsEntryId::HispeedAutoAdjust,
                ],
            ),
        ];

        for (preset, mut expected) in cases {
            profile.lane.set_hispeed_config(preset);
            let items = load_settings_items_for_config(
                CONFIG_DISPLAY_PATH,
                AppLocale::Ja,
                &app_config,
                &profile,
                &SettingsFolderExtras::EMPTY,
            );
            let actual = items
                .iter()
                .filter_map(|item| match item {
                    SelectItem::Config(row) => Some(row.entry_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            expected.extend(common);
            assert_eq!(actual, expected, "{preset:?}");
        }
    }

    #[test]
    fn settings_input_lists_supported_entries() {
        let items = load_settings_items(CONFIG_INPUT_PATH);
        assert_eq!(
            items.len(),
            SettingsEntryId::INPUT_ENTRIES.len()
                + AppSettingsEntryId::INPUT_ENTRIES.len()
                + 2
                + 3
                + 1
        );
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row)
                if row.entry_id == SettingsEntryId::AnalogScratchSensitivity1P
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::KeyboardReleaseBounceMs
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::ControllerReleaseBounceMs
        )));
    }

    #[test]
    fn settings_replay_lists_slot_rules() {
        let items = load_settings_items(CONFIG_REPLAY_PATH);
        assert_eq!(items.len(), SettingsEntryId::REPLAY_ENTRIES.len() + 1);
        assert!(matches!(items.first(), Some(SelectItem::SettingsBack)));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::ReplaySlot4Rule
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::ReplayCompress
        )));
    }

    #[test]
    fn settings_ui_lists_language_and_fps() {
        let items = load_settings_items(CONFIG_UI_PATH);
        assert_eq!(items.len(), SettingsEntryId::UI_ENTRIES.len() + 1);
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::Language
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            SelectItem::Config(row) if row.entry_id == SettingsEntryId::ShowFps
        )));
    }

    #[test]
    fn every_setting_entry_has_a_localized_item_description() {
        let entry_sets = [
            SettingsEntryId::VOLUME_ENTRIES,
            SettingsEntryId::JUDGE_ENTRIES,
            SettingsEntryId::PLAY_ENTRIES,
            SettingsEntryId::SEVEN_TO_NINE_ENTRIES,
            SettingsEntryId::ASSIST_ENTRIES,
            SettingsEntryId::ASSIST_NOTE_ENTRIES,
            SettingsEntryId::ASSIST_JUDGE_ENTRIES,
            SettingsEntryId::DISPLAY_ENTRIES,
            SettingsEntryId::INPUT_ENTRIES,
            SettingsEntryId::SELECT_ENTRIES,
            SettingsEntryId::HISPEED_8K_ENTRIES,
            SettingsEntryId::REPLAY_ENTRIES,
            SettingsEntryId::UI_ENTRIES,
            SettingsEntryId::IR_ENTRIES,
        ];
        let mut profile = ProfileConfig::new_default("default", "Default", 0);

        for locale in AppLocale::SUPPORTED {
            profile.ui.language = locale.code().to_string();
            for entry_id in entry_sets
                .into_iter()
                .flatten()
                .copied()
                .chain(std::iter::once(SettingsEntryId::Assist))
            {
                let key = entry_id.description_key();
                let description = ConfigSelectRow { entry_id }.description_text(&profile);
                assert_ne!(description, key, "{}: {entry_id:?}", locale.code());
                assert!(!description.is_empty(), "{}: {entry_id:?}", locale.code());
            }
        }
    }

    #[test]
    fn every_app_setting_entry_has_a_localized_item_description() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let mut entries = AppSettingsEntryId::VIDEO_ENTRIES.to_vec();
        entries.extend([
            AppSettingsEntryId::AudioBackend,
            AppSettingsEntryId::AudioOutputMode,
            AppSettingsEntryId::AudioSampleRate,
            AppSettingsEntryId::AudioBufferMode,
            AppSettingsEntryId::AudioBufferSize,
            AppSettingsEntryId::AudioOutputDevice,
            AppSettingsEntryId::AudioAsioDriver,
            AppSettingsEntryId::AudioOutputChannelPair,
        ]);
        for list in [
            AppSettingsEntryId::INPUT_ENTRIES,
            AppSettingsEntryId::SELECT_ENTRIES,
            AppSettingsEntryId::SCAN_ENTRIES,
            AppSettingsEntryId::TABLES_ENTRIES,
            AppSettingsEntryId::DOWNLOADS_ENTRIES,
            AppSettingsEntryId::SYSTEM_ENTRIES,
        ] {
            entries.extend_from_slice(list);
        }
        for locale in AppLocale::SUPPORTED {
            let mut localized = profile.clone();
            localized.ui.language = locale.code().to_string();
            for entry_id in &entries {
                let description =
                    AppConfigSelectRow { entry_id: *entry_id }.description_text(&localized);
                assert_ne!(description, entry_id.description_key());
            }
        }
    }

    fn choice_rows(items: &[SelectItem]) -> Vec<&SettingsChoiceRow> {
        items
            .iter()
            .filter_map(|item| match item {
                SelectItem::SettingsChoice(row) => Some(row),
                _ => None,
            })
            .collect()
    }

    fn action_kinds(items: &[SelectItem]) -> Vec<SettingsActionKind> {
        items
            .iter()
            .filter_map(|item| match item {
                SelectItem::SettingsAction(row) => Some(row.kind),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn controller_only_folders_parse_and_are_listed_at_the_root() {
        for (path, expected) in [
            (CONFIG_PROFILE_PATH, SettingsPath::Profile),
            (CONFIG_SKIN_PATH, SettingsPath::SkinRoot),
            (CONFIG_LIBRARY_PATH, SettingsPath::Library),
            (CONFIG_LIBRARY_SONGS_PATH, SettingsPath::LibrarySongs),
            (CONFIG_LIBRARY_TABLES_PATH, SettingsPath::LibraryTables),
            (CONFIG_LIBRARY_DOWNLOADS_PATH, SettingsPath::LibraryDownloads),
            (CONFIG_IR_PATH, SettingsPath::Ir),
            (CONFIG_SYSTEM_PATH, SettingsPath::System),
        ] {
            assert_eq!(parse_settings_path(path), Some(expected), "{path}");
            assert_ne!(settings_breadcrumb(path), settings_breadcrumb(CONFIG_ROOT_PATH));
        }
        for target in Lr2SkinTarget::ALL {
            let path = format!("{CONFIG_SKIN_SLOT_PREFIX}{}", target.key());
            assert_eq!(parse_settings_path(&path), Some(SettingsPath::Skin(target)));
            assert_eq!(settings_skin_target(&path), Some(target));
        }
        assert_eq!(parse_settings_path("bmz-settings:skin:unknown"), None);

        let root = load_settings_items(CONFIG_ROOT_PATH);
        for path in [
            CONFIG_PROFILE_PATH,
            CONFIG_SKIN_PATH,
            CONFIG_LIBRARY_PATH,
            CONFIG_IR_PATH,
            CONFIG_SYSTEM_PATH,
        ] {
            assert!(
                root.iter()
                    .any(|item| matches!(item, SelectItem::Folder { path: p, .. } if p == path)),
                "{path}"
            );
        }
        let skins = load_settings_items(CONFIG_SKIN_PATH);
        assert_eq!(skins.len(), Lr2SkinTarget::ALL.len() + 1);
    }

    #[test]
    fn profile_folder_lists_profiles_and_keeps_the_current_one() {
        let profile = ProfileConfig::new_default("main", "Main", 0);
        let extras = SettingsFolderExtras {
            profiles: vec![("sub".into(), "Sub".into()), ("alt".into(), "alt".into())],
            ..Default::default()
        };
        let items = load_settings_items_for_config(
            CONFIG_PROFILE_PATH,
            AppLocale::Ja,
            &AppConfig::default(),
            &profile,
            &extras,
        );
        let rows = choice_rows(&items);
        assert_eq!(rows.len(), 1);
        let row = rows[0];
        assert_eq!(row.target, SettingsChoiceTarget::Profile);
        assert_eq!(
            row.choices.iter().map(|choice| choice.label.as_str()).collect::<Vec<_>>(),
            ["Main (main)", "Sub (sub)", "alt"]
        );
        assert_eq!(row.value_text(), "Main (main)");
    }

    #[test]
    fn choice_rows_wrap_in_both_directions() {
        let mut row = SettingsChoiceRow {
            target: SettingsChoiceTarget::Profile,
            label: String::new(),
            description: String::new(),
            choices: vec![SettingsChoice::new("a", "A"), SettingsChoice::new("b", "B")],
            current: 0,
        };
        assert!(row.step(-1));
        assert_eq!(row.current, 1);
        assert!(row.step(5));
        assert_eq!(row.current, 0);
        assert!(!row.step(0));
        row.choices.truncate(1);
        assert!(!row.step(1));
        row.choices.clear();
        assert_eq!(row.value_text(), "-");
    }

    #[test]
    fn skin_folder_needs_the_slot_data() {
        let target = Lr2SkinTarget::from_key("play7").unwrap();
        let path = format!("{CONFIG_SKIN_SLOT_PREFIX}play7");
        assert!(load_settings_items(&path).is_empty());

        let data = SkinFolderData {
            target,
            skins: vec![SettingsChoice::new("", "Default"), SettingsChoice::new("a.json", "A")],
            current_skin: 1,
            items: vec![(
                "Lane".to_string(),
                vec![SettingsChoice::new("Wide", "Wide"), SettingsChoice::new("Thin", "Thin")],
                1,
            )],
        };
        let extras = SettingsFolderExtras { skin: Some(data.clone()), ..Default::default() };
        let items = load_settings_items_for_config(
            &path,
            AppLocale::En,
            &AppConfig::default(),
            &ProfileConfig::new_default("default", "Default", 0),
            &extras,
        );
        let rows = choice_rows(&items);
        assert_eq!(rows[0].target, SettingsChoiceTarget::SkinPath(target));
        assert_eq!(rows[0].value_text(), "A");
        assert_eq!(rows[1].target, SettingsChoiceTarget::SkinItem(target, 0));
        assert_eq!((rows[1].label.as_str(), rows[1].value_text().as_str()), ("Lane", "Thin"));

        // Data for another slot is not shown.
        let other = format!("{CONFIG_SKIN_SLOT_PREFIX}select");
        assert!(
            load_settings_items_for_config(
                &other,
                AppLocale::En,
                &AppConfig::default(),
                &ProfileConfig::new_default("default", "Default", 0),
                &extras,
            )
            .is_empty()
        );
    }

    #[test]
    fn library_folders_reflect_song_roots_tables_and_actions() {
        use crate::config::app_config::{DifficultyTableSource, PathEntry};
        let mut config = AppConfig::default();
        config.songs.roots =
            vec![PathEntry { path: "/music/bms/".into(), enabled: false, recursive: true }];
        config.tables.sources = vec![
            DifficultyTableSource { url: "https://a.example/".into(), enabled: true },
            DifficultyTableSource { url: "https://b.example/".into(), enabled: false },
        ];
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let extras = SettingsFolderExtras {
            table_names: vec![("https://a.example/".into(), "Table A".into())],
            ..Default::default()
        };
        let load =
            |path| load_settings_items_for_config(path, AppLocale::En, &config, &profile, &extras);

        let songs = load(CONFIG_LIBRARY_SONGS_PATH);
        let rows = choice_rows(&songs);
        assert_eq!(rows[0].target, SettingsChoiceTarget::SongRootEnabled(0));
        assert_eq!((rows[0].label.as_str(), rows[0].value_text().as_str()), ("/music/bms/", "OFF"));
        assert_eq!(rows[1].target, SettingsChoiceTarget::SongRootRecursive(0));
        assert!(rows[1].label.ends_with("bms"), "{}", rows[1].label);
        assert_eq!(rows[1].value_text(), "ON");
        assert_eq!(
            action_kinds(&songs),
            [SettingsActionKind::RescanSongs, SettingsActionKind::FullRescanSongs]
        );

        let tables = load(CONFIG_LIBRARY_TABLES_PATH);
        let rows = choice_rows(&tables);
        assert_eq!((rows[0].label.as_str(), rows[0].value_text().as_str()), ("Table A", "ON"));
        assert_eq!(
            (rows[1].label.as_str(), rows[1].value_text().as_str()),
            ("https://b.example/", "OFF")
        );
        assert_eq!(
            action_kinds(&tables),
            [
                SettingsActionKind::FetchEnabledTables,
                SettingsActionKind::FetchTable(0),
                SettingsActionKind::FetchTable(1),
            ]
        );

        let downloads = load(CONFIG_LIBRARY_DOWNLOADS_PATH);
        assert_eq!(downloads.len(), AppSettingsEntryId::DOWNLOADS_ENTRIES.len() + 1);
    }

    #[test]
    fn input_folder_lists_controller_slots_and_keeps_a_disconnected_device() {
        let mut config = AppConfig::default();
        config.input.gamepad_slot_device_ids =
            [Some("pad-a".into()), Some("missing-controller-with-a-long-id".into())];
        let extras = SettingsFolderExtras {
            gamepads: vec![("pad-a".into(), "#0 Pad A".into())],
            ..Default::default()
        };
        let items = load_settings_items_for_config(
            CONFIG_INPUT_PATH,
            AppLocale::En,
            &config,
            &ProfileConfig::new_default("default", "Default", 0),
            &extras,
        );
        let rows = choice_rows(&items);
        assert_eq!(rows[0].target, SettingsChoiceTarget::GamepadSlot(0));
        assert_eq!(rows[0].value_text(), "#0 Pad A");
        assert_eq!(rows[1].target, SettingsChoiceTarget::GamepadSlot(1));
        assert_eq!(rows[1].choices.len(), 3);
        assert_eq!(rows[1].choices[rows[1].current].key, "missing-controller-with-a-long-id");
        assert_eq!(
            action_kinds(&items),
            [
                SettingsActionKind::GamepadAutoAssign,
                SettingsActionKind::GamepadSwap,
                SettingsActionKind::GamepadClearAssignment,
            ]
        );
    }

    #[test]
    fn key_folders_offer_restore_defaults_for_each_used_slot() {
        let kinds = action_kinds(&load_settings_items(CONFIG_KEYS_COMMON_PATH));
        assert!(kinds.contains(&SettingsActionKind::RestoreKeyDefaults {
            scope: KeyDefaultsScope::Group(KeyBindingGroup::Common),
            slot: KeyBindingSlot::Controller,
        }));
        let kinds = action_kinds(&load_settings_items("bmz-settings:keys:14k"));
        assert!(kinds.contains(&SettingsActionKind::RestoreKeyDefaults {
            scope: KeyDefaultsScope::Mode(KeyMode::K14),
            slot: KeyBindingSlot::Controller,
        }));
    }
}
