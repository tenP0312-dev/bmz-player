use std::collections::HashSet;

use bmz_core::lane::KeyMode;

use crate::config::app_config::{
    AppConfig, AudioConfig, ChartDownloadsConfig, GlobalInputConfig, LoggingConfig,
    MusicSelectConfig, ScanConfig, ScreenshotConfig, VideoConfig,
};
use crate::config::app_settings_registry::{
    AppSettingsChoices, AppSettingsEntryId, AppSettingsSection, adjust_app_settings_value,
};
use crate::config::play_input::resolve_play_bindings;
use crate::config::profile_config::{
    InputActionConfig, LaneConfig, ProfileConfig, ProfileInputConfig, ScratchDirectionConfig,
    SelectInputModeConfig,
};
use crate::config::settings_registry::{
    SettingsEntryId, adjust_settings_value, format_settings_value,
};

/// 7KEY / 14KEY + スクラッチ向けの設定画面入力マッピング。
#[derive(Debug, Clone)]
pub struct SettingsBindings {
    confirm: HashSet<String>,
    back: HashSet<String>,
    increase: HashSet<String>,
    decrease: HashSet<String>,
}

impl SettingsBindings {
    pub fn from_profile(input: &ProfileInputConfig) -> Self {
        let mut confirm = HashSet::new();
        let mut back = HashSet::new();
        let mut increase = HashSet::new();
        let mut decrease = HashSet::new();

        let mut play_controls = HashSet::new();
        match input.select_input_mode {
            SelectInputModeConfig::Key7Key14 => {
                collect_play_settings_bindings(
                    input,
                    KeyMode::K7,
                    &mut confirm,
                    &mut back,
                    &mut increase,
                    &mut decrease,
                );
                collect_play_settings_bindings(
                    input,
                    KeyMode::K14,
                    &mut confirm,
                    &mut back,
                    &mut increase,
                    &mut decrease,
                );
            }
            SelectInputModeConfig::Key9 => {
                collect_play_9k_settings_bindings(
                    input,
                    &mut confirm,
                    &mut back,
                    &mut increase,
                    &mut decrease,
                    &mut play_controls,
                );
            }
        }

        for entry in &input.ui.bindings {
            if input.select_input_mode == SelectInputModeConfig::Key9
                && play_controls.contains(&entry.control)
            {
                continue;
            }
            if entry.action == Some(InputActionConfig::E2) {
                back.insert(entry.control.clone());
            }
        }

        for key in ["Enter", "Space", "ArrowRight"] {
            confirm.insert(key.to_string());
        }
        for key in ["ArrowLeft", "Escape"] {
            back.insert(key.to_string());
        }
        for key in ["ArrowUp", "ScratchDown"] {
            increase.insert(key.to_string());
        }
        for key in ["ArrowDown", "ScratchUp"] {
            decrease.insert(key.to_string());
        }

        Self { confirm, back, increase, decrease }
    }

    pub fn is_confirm(&self, control: &str) -> bool {
        self.confirm.contains(control)
    }

    pub fn is_back(&self, control: &str) -> bool {
        self.back.contains(control)
    }

    pub fn is_increase(&self, control: &str) -> bool {
        self.increase.contains(control)
    }

    pub fn is_decrease(&self, control: &str) -> bool {
        self.decrease.contains(control)
    }
}

fn collect_play_9k_settings_bindings(
    input: &ProfileInputConfig,
    confirm: &mut HashSet<String>,
    back: &mut HashSet<String>,
    increase: &mut HashSet<String>,
    decrease: &mut HashSet<String>,
    play_controls: &mut HashSet<String>,
) {
    let Ok(play) = resolve_play_bindings(input, KeyMode::K9) else {
        return;
    };
    for entry in play {
        play_controls.insert(entry.control.clone());
        let Some(lane) = entry.lane else { continue };
        match lane {
            LaneConfig::Key3 => {
                back.insert(entry.control.clone());
            }
            LaneConfig::Key4 => {
                decrease.insert(entry.control.clone());
            }
            LaneConfig::Key5 | LaneConfig::Key7 => {
                confirm.insert(entry.control.clone());
            }
            LaneConfig::Key6 => {
                increase.insert(entry.control.clone());
            }
            _ => {}
        }
    }
}

fn collect_play_settings_bindings(
    input: &ProfileInputConfig,
    key_mode: KeyMode,
    confirm: &mut HashSet<String>,
    back: &mut HashSet<String>,
    increase: &mut HashSet<String>,
    decrease: &mut HashSet<String>,
) {
    let Ok(play) = resolve_play_bindings(input, key_mode) else {
        return;
    };
    for entry in play {
        let Some(lane) = entry.lane else { continue };
        match lane {
            LaneConfig::Key1
            | LaneConfig::Key3
            | LaneConfig::Key5
            | LaneConfig::Key7
            | LaneConfig::Key8
            | LaneConfig::Key10
            | LaneConfig::Key12
            | LaneConfig::Key14 => {
                confirm.insert(entry.control.clone());
            }
            LaneConfig::Key2
            | LaneConfig::Key4
            | LaneConfig::Key6
            | LaneConfig::Key9
            | LaneConfig::Key11
            | LaneConfig::Key13 => {
                back.insert(entry.control.clone());
            }
            LaneConfig::Scratch | LaneConfig::Scratch2 => match entry.scratch {
                Some(ScratchDirectionConfig::Down) => {
                    increase.insert(entry.control.clone());
                }
                Some(ScratchDirectionConfig::Up) => {
                    decrease.insert(entry.control.clone());
                }
                None => {
                    classify_scratch_control(&entry.control, increase, decrease);
                }
            },
        }
    }
}

fn classify_scratch_control(
    control: &str,
    increase: &mut HashSet<String>,
    decrease: &mut HashSet<String>,
) {
    if control.contains("ScratchDown")
        || control.ends_with('+')
        || control == "Axis1+"
        || control == "Button8"
    {
        increase.insert(control.to_string());
        return;
    }
    if control.contains("ScratchUp")
        || control.ends_with('-')
        || control == "Axis1-"
        || control == "Button9"
    {
        decrease.insert(control.to_string());
        return;
    }
    increase.insert(control.to_string());
    decrease.insert(control.to_string());
}

/// Captures only the setting being edited, including its dependent fields.
#[derive(Debug, Clone)]
pub struct SettingsEditSession {
    baseline: crate::config::settings_registry::SettingsSnapshot,
}

#[derive(Debug, Clone)]
enum AppSettingsBaseline {
    Audio(AudioConfig),
    Video(VideoConfig),
    Input(GlobalInputConfig),
    Select(MusicSelectConfig),
    Scan(ScanConfig),
    TablesAutoFetch(bool),
    Downloads(ChartDownloadsConfig),
    Screenshot(ScreenshotConfig),
    Logging(LoggingConfig),
}

/// `data/config.toml` の設定を一項目ずつ編集するセッション。
/// 取消では、編集した項目のセクションだけを戻す。
#[derive(Debug, Clone)]
pub struct AppSettingsEditSession {
    pub entry_id: AppSettingsEntryId,
    baseline: AppSettingsBaseline,
    choices: AppSettingsChoices,
}

impl AppSettingsEditSession {
    pub fn capture(
        config: &AppConfig,
        entry_id: AppSettingsEntryId,
        choices: AppSettingsChoices,
    ) -> Self {
        let baseline = match entry_id.section() {
            AppSettingsSection::Audio => AppSettingsBaseline::Audio(config.audio.clone()),
            AppSettingsSection::Video => AppSettingsBaseline::Video(config.video.clone()),
            AppSettingsSection::Input => AppSettingsBaseline::Input(config.input.clone()),
            AppSettingsSection::Select => AppSettingsBaseline::Select(config.select.clone()),
            AppSettingsSection::Scan => AppSettingsBaseline::Scan(config.scan.clone()),
            AppSettingsSection::Tables => {
                AppSettingsBaseline::TablesAutoFetch(config.tables.auto_fetch_on_startup)
            }
            AppSettingsSection::Downloads => {
                AppSettingsBaseline::Downloads(config.downloads.clone())
            }
            AppSettingsSection::Screenshot => {
                AppSettingsBaseline::Screenshot(config.screenshot.clone())
            }
            AppSettingsSection::Logging => AppSettingsBaseline::Logging(config.logging.clone()),
        };
        Self { entry_id, baseline, choices }
    }

    pub fn restore(&self, config: &mut AppConfig) {
        match &self.baseline {
            AppSettingsBaseline::Audio(value) => config.audio = value.clone(),
            AppSettingsBaseline::Video(value) => config.video = value.clone(),
            AppSettingsBaseline::Input(value) => config.input = value.clone(),
            AppSettingsBaseline::Select(value) => config.select = value.clone(),
            AppSettingsBaseline::Scan(value) => config.scan = value.clone(),
            AppSettingsBaseline::TablesAutoFetch(value) => {
                config.tables.auto_fetch_on_startup = *value;
            }
            AppSettingsBaseline::Downloads(value) => config.downloads = value.clone(),
            AppSettingsBaseline::Screenshot(value) => config.screenshot = value.clone(),
            AppSettingsBaseline::Logging(value) => config.logging = value.clone(),
        }
    }

    /// The input settings before this edit, to apply only a changed backend or device.
    pub fn input_baseline(&self) -> Option<&GlobalInputConfig> {
        match &self.baseline {
            AppSettingsBaseline::Input(value) => Some(value),
            _ => None,
        }
    }

    /// The select settings before this edit.
    pub fn select_baseline(&self) -> Option<&MusicSelectConfig> {
        match &self.baseline {
            AppSettingsBaseline::Select(value) => Some(value),
            _ => None,
        }
    }

    pub fn adjust(&self, config: &mut AppConfig, direction: i32) -> bool {
        adjust_app_settings_value(config, self.entry_id, &self.choices, direction)
    }
}

/// 選曲設定で現在編集中の保存先を表す。
#[derive(Debug, Clone)]
pub enum SelectSettingsEditSession {
    Profile(SettingsEditSession),
    App(AppSettingsEditSession),
    Choice(ChoiceEditSession),
}

/// A row whose values are listed when the folder is built (profiles, skins,
/// song folders, ...). Editing only moves the draft; nothing is written until
/// the edit is confirmed, so cancelling needs no restore.
#[derive(Debug, Clone)]
pub struct ChoiceEditSession {
    /// Index of the edited row in the select list, to show the draft value.
    pub item_index: usize,
    pub row: crate::screens::settings_model::SettingsChoiceRow,
    original: usize,
}

impl ChoiceEditSession {
    pub fn begin(
        item_index: usize,
        row: crate::screens::settings_model::SettingsChoiceRow,
    ) -> Self {
        let original = row.current;
        Self { item_index, row, original }
    }

    pub fn adjust(&mut self, direction: i32) -> bool {
        self.row.step(direction)
    }

    /// The confirmed choice, or `None` when the value did not change.
    pub fn changed_choice(&self) -> Option<&crate::screens::settings_model::SettingsChoice> {
        (self.row.current != self.original).then(|| self.row.choices.get(self.row.current))?
    }

    pub fn original(&self) -> usize {
        self.original
    }
}

impl SettingsEditSession {
    pub fn capture(profile: &ProfileConfig, entry_id: SettingsEntryId) -> Self {
        Self { baseline: entry_id.capture(profile) }
    }

    pub fn entry_id(&self) -> SettingsEntryId {
        self.baseline.entry_id()
    }

    pub fn restore(&self, profile: &mut ProfileConfig) {
        self.baseline.restore(profile);
    }

    pub fn preview_value(&self, profile: &ProfileConfig) -> String {
        format_settings_value(profile, self.entry_id())
    }
}

pub fn adjust_settings_draft(
    profile: &mut ProfileConfig,
    session: &SettingsEditSession,
    delta: i32,
) -> bool {
    adjust_settings_value(profile, session.entry_id(), delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::app_config::AppConfig;
    use crate::config::profile_config::{
        DifficultyTableLevelDisplay, DoubleOptionConfig, GaugeTypeConfig, KeyModeConversionConfig,
        ProfileConfig, ReplaySlotRule,
    };
    use crate::select_options::SessionMode;

    #[test]
    fn every_setting_cancel_restores_its_values_and_preserves_unrelated_edits() {
        let mut default = ProfileConfig::new_default("default", "Default", 0);
        // Direction editing materializes this optional section even when the
        // effective default is restored. Compare explicit sections on both sides.
        default.input.play.entry(KeyMode::K8.play_map_key().to_string()).or_default();
        let mut adjusted = default.clone();
        for &entry in SettingsEntryId::ALL {
            adjust_settings_value(&mut adjusted, entry, 1);
        }
        for original in [default, adjusted] {
            for &entry in SettingsEntryId::ALL {
                for delta in [-10_000, -1, 0, 1, 10_000] {
                    let mut profile = original.clone();
                    let session = SettingsEditSession::capture(&profile, entry);
                    assert_eq!(session.entry_id(), entry);
                    let display = session.preview_value(&profile);
                    adjust_settings_draft(&mut profile, &session, delta);
                    profile.display_name = "Changed independently".to_string();
                    session.restore(&mut profile);
                    let mut expected = original.clone();
                    expected.display_name.clone_from(&profile.display_name);
                    assert_eq!(
                        serde_json::to_value(&profile).unwrap(),
                        serde_json::to_value(&expected).unwrap(),
                        "cancel {entry:?}, delta={delta}",
                    );
                    assert_eq!(session.preview_value(&profile), display, "{entry:?}");
                }
            }
        }
    }

    #[test]
    fn default_7k_bindings_map_scratch_and_keys() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let bindings = SettingsBindings::from_profile(&profile.input);

        assert!(bindings.is_confirm("Z"));
        assert!(bindings.is_confirm("C"));
        assert!(bindings.is_back("S"));
        assert!(bindings.is_back("D"));
        assert!(bindings.is_increase("Axis1-") || bindings.is_increase("LControl"));
    }

    #[test]
    fn default_14k_2p_bindings_map_scratch_and_keys() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let bindings = SettingsBindings::from_profile(&profile.input);

        assert!(bindings.is_confirm("M"));
        assert!(bindings.is_confirm("Period"));
        assert!(bindings.is_confirm("Slash"));
        assert!(bindings.is_back("K"));
        assert!(bindings.is_back("L"));
        assert!(bindings.is_back("Semicolon"));
        assert!(bindings.is_decrease("RShift"));
        assert!(bindings.is_increase("RControl"));
    }

    #[test]
    fn cursor_up_increases_and_down_decreases_settings_values() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let bindings = SettingsBindings::from_profile(&profile.input);

        assert!(bindings.is_increase("ArrowUp"));
        assert!(!bindings.is_decrease("ArrowUp"));
        assert!(bindings.is_decrease("ArrowDown"));
        assert!(!bindings.is_increase("ArrowDown"));
    }

    #[test]
    fn key9_select_input_maps_settings_navigation_keys() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        profile.input.select_input_mode = SelectInputModeConfig::Key9;
        let bindings = SettingsBindings::from_profile(&profile.input);

        assert!(bindings.is_confirm("C"));
        assert!(bindings.is_confirm("V"));
        assert!(bindings.is_back("X"));
        assert!(bindings.is_decrease("D"));
        assert!(bindings.is_increase("F"));
        assert!(!bindings.is_confirm("Z"));
        assert!(!bindings.is_back("S"));
    }

    #[test]
    fn gamepad_button1_confirms_only_through_lane_bindings() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        assert!(SettingsBindings::from_profile(&profile.input).is_confirm("Button1"));

        let mut nine_key = profile.clone();
        nine_key.input.select_input_mode = SelectInputModeConfig::Key9;
        let play9 = nine_key.input.play.entry(KeyMode::K9.play_map_key().to_string()).or_default();
        play9.bindings.push(crate::config::profile_config::BindingConfigEntry {
            device: "gamepad".to_string(),
            control: "Button1".to_string(),
            keyboard_slot: None,
            lane: Some(LaneConfig::Key1),
            action: None,
            scratch: None,
        });
        assert!(!SettingsBindings::from_profile(&nine_key.input).is_confirm("Button1"));

        let defaults = SettingsBindings::from_profile(&profile.input);
        assert!(!defaults.is_back("Select"));
        assert!(!defaults.is_increase("DPadDown"));
        assert!(!defaults.is_decrease("DPadUp"));
    }

    #[test]
    fn edit_session_restore_reverts_volume() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        let session = SettingsEditSession::capture(&profile, SettingsEntryId::MasterVolume);
        profile.audio_mix.master_volume = 20;
        session.restore(&mut profile);
        assert_eq!(profile.audio_mix.master_volume, 50);

        let normalize_session =
            SettingsEditSession::capture(&profile, SettingsEntryId::NormalizeChartVolume);
        profile.audio_mix.normalize_chart_volume = false;
        normalize_session.restore(&mut profile);
        assert!(profile.audio_mix.normalize_chart_volume);

        let system_bgm_session =
            SettingsEditSession::capture(&profile, SettingsEntryId::NormalizeSystemBgmVolume);
        profile.audio_mix.normalize_system_bgm_volume = false;
        system_bgm_session.restore(&mut profile);
        assert!(profile.audio_mix.normalize_system_bgm_volume);
    }

    #[test]
    fn app_edit_session_restores_the_edited_config_section() {
        let mut config = AppConfig::default();
        let audio = AppSettingsEditSession::capture(
            &config,
            AppSettingsEntryId::AudioBufferSize,
            AppSettingsChoices::None,
        );
        config.audio.buffer_size = 64;
        audio.restore(&mut config);
        assert_eq!(config.audio.buffer_size, 256);

        let video = AppSettingsEditSession::capture(
            &config,
            AppSettingsEntryId::VideoTargetFps,
            AppSettingsChoices::None,
        );
        config.video.target_fps = 0;
        video.restore(&mut config);
        assert_eq!(config.video.target_fps, 240);
    }

    #[test]
    fn edit_session_restore_reverts_gauge() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        let session = SettingsEditSession::capture(&profile, SettingsEntryId::Gauge);
        profile.play.gauge = GaugeTypeConfig::Hazard;
        session.restore(&mut profile);
        assert_eq!(profile.play.gauge, GaugeTypeConfig::Normal);
    }

    #[test]
    fn edit_session_restore_reverts_input_and_replay_settings() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        let table_level_session =
            SettingsEditSession::capture(&profile, SettingsEntryId::DifficultyTableLevelDisplay);
        profile.select.difficulty_table_level_display = DifficultyTableLevelDisplay::Chart;
        table_level_session.restore(&mut profile);
        assert_eq!(
            profile.select.difficulty_table_level_display,
            DifficultyTableLevelDisplay::Table
        );

        let analog_2p_session =
            SettingsEditSession::capture(&profile, SettingsEntryId::AnalogScratch2P);
        profile.input.gamepad2.analog_scratch = false;
        analog_2p_session.restore(&mut profile);
        assert!(profile.input.gamepad2.analog_scratch);

        let controller_bounce_session =
            SettingsEditSession::capture(&profile, SettingsEntryId::ControllerReleaseBounceMs);
        profile.input.controller_release_bounce_ms = 12;
        controller_bounce_session.restore(&mut profile);
        assert_eq!(profile.input.controller_release_bounce_ms, 0);

        let replay_session =
            SettingsEditSession::capture(&profile, SettingsEntryId::ReplaySlot2Rule);
        profile.replay.slot_rules[1] = ReplaySlotRule::ClearUpdate;
        replay_session.restore(&mut profile);
        assert_eq!(profile.replay.slot_rules[1], ReplaySlotRule::ScoreUpdate);
    }

    #[test]
    fn edit_session_restore_reverts_new_dependent_settings() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);

        let session_mode = SettingsEditSession::capture(&profile, SettingsEntryId::SessionMode);
        assert!(adjust_settings_draft(&mut profile, &session_mode, 1));
        assert!(adjust_settings_draft(&mut profile, &session_mode, 1));
        assert_eq!(profile.play.session_mode, Some(SessionMode::Autoplay));
        assert!(profile.play.auto_play);
        session_mode.restore(&mut profile);
        assert_eq!(profile.play.session_mode, Some(SessionMode::Normal));
        assert!(!profile.play.auto_play);

        profile.play.double_option = DoubleOptionConfig::Battle;
        let conversion = SettingsEditSession::capture(&profile, SettingsEntryId::KeyModeConversion);
        assert!(adjust_settings_draft(&mut profile, &conversion, 1));
        assert_eq!(profile.play.double_option, DoubleOptionConfig::Off);
        conversion.restore(&mut profile);
        assert_eq!(profile.play.key_mode_conversion, KeyModeConversionConfig::Off);
        assert_eq!(profile.play.double_option, DoubleOptionConfig::Battle);

        let assist = SettingsEditSession::capture(&profile, SettingsEntryId::AssistScrollRate);
        assert!(adjust_settings_draft(&mut profile, &assist, 5));
        assist.restore(&mut profile);
        assert!((profile.play.assist.scroll_rate - 0.5).abs() < f64::EPSILON);

        let note_retention = SettingsEditSession::capture(&profile, SettingsEntryId::NoteRetention);
        assert!(adjust_settings_draft(&mut profile, &note_retention, 1));
        assert!(profile.play.note_retention);
        note_retention.restore(&mut profile);
        assert!(!profile.play.note_retention);

        let duration =
            SettingsEditSession::capture(&profile, SettingsEntryId::NoteDisplayDurationMs);
        assert!(adjust_settings_draft(&mut profile, &duration, 10));
        assert_eq!(profile.lane.target_green_number, 306);
        duration.restore(&mut profile);
        assert_eq!(profile.lane.target_green_number, 300);

        let language = SettingsEditSession::capture(&profile, SettingsEntryId::Language);
        assert!(adjust_settings_draft(&mut profile, &language, 1));
        language.restore(&mut profile);
        assert_eq!(profile.ui.language, "ja");
    }

    #[test]
    fn random_select_settings_toggle_independently_and_cancel() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        let entries = [
            SettingsEntryId::SelectRandomSelect,
            SettingsEntryId::SelectRandomNoPlay,
            SettingsEntryId::SelectRandomFailed,
            SettingsEntryId::SelectRandomNotEasy,
            SettingsEntryId::SelectRandomNotClear,
            SettingsEntryId::SelectRandomNotHard,
            SettingsEntryId::SelectRandomNotExHard,
            SettingsEntryId::SelectRandomNotFullCombo,
        ];
        for (index, entry) in entries.into_iter().enumerate() {
            assert!(SettingsEntryId::SELECT_ENTRIES.contains(&entry));
            let session = SettingsEditSession::capture(&profile, entry);
            assert!(!adjust_settings_draft(&mut profile, &session, 0));
            assert!(adjust_settings_draft(&mut profile, &session, 1));
            let mut expected = [false; 8];
            expected[index] = true;
            assert_eq!(profile.select.random_select_flags(), expected);
            session.restore(&mut profile);
            assert_eq!(profile.select.random_select_flags(), [false; 8]);
        }
    }

    #[test]
    fn choice_edit_reports_a_change_only_when_the_value_moved() {
        use crate::screens::settings_model::{
            SettingsChoice, SettingsChoiceRow, SettingsChoiceTarget,
        };
        let row = SettingsChoiceRow {
            target: SettingsChoiceTarget::TableEnabled(0),
            label: "Table".into(),
            description: String::new(),
            choices: vec![SettingsChoice::new("off", "OFF"), SettingsChoice::new("on", "ON")],
            current: 1,
        };
        let mut session = ChoiceEditSession::begin(3, row);
        assert_eq!(session.item_index, 3);
        assert!(session.changed_choice().is_none());
        assert!(session.adjust(1));
        assert_eq!(session.changed_choice().map(|choice| choice.key.as_str()), Some("off"));
        assert!(session.adjust(-1));
        assert!(session.changed_choice().is_none());
        assert_eq!(session.original(), 1);
    }

    #[test]
    fn app_edit_session_restores_only_the_new_sections() {
        let mut config = AppConfig::default();
        let input = AppSettingsEditSession::capture(
            &config,
            AppSettingsEntryId::InputKeyboardEnabled,
            AppSettingsChoices::None,
        );
        assert!(input.adjust(&mut config, 1));
        config.downloads.http_parallelism = 5;
        input.restore(&mut config);
        assert!(config.input.keyboard_enabled);
        assert_eq!(config.downloads.http_parallelism, 5, "other sections stay");
        assert!(input.input_baseline().is_some());
        assert!(input.select_baseline().is_none());

        let downloads = AppSettingsEditSession::capture(
            &config,
            AppSettingsEntryId::DownloadsHttpParallelism,
            AppSettingsChoices::None,
        );
        assert!(downloads.adjust(&mut config, -1));
        downloads.restore(&mut config);
        assert_eq!(config.downloads.http_parallelism, 5);

        let tables = AppSettingsEditSession::capture(
            &config,
            AppSettingsEntryId::TablesFetchOnStartup,
            AppSettingsChoices::None,
        );
        let before = config.tables.auto_fetch_on_startup;
        let source_count = config.tables.sources.len();
        assert!(tables.adjust(&mut config, 1));
        config.tables.sources.push(crate::config::app_config::DifficultyTableSource {
            url: "https://example.com/".into(),
            enabled: true,
        });
        tables.restore(&mut config);
        assert_eq!(config.tables.auto_fetch_on_startup, before);
        assert_eq!(config.tables.sources.len(), source_count + 1, "table list edits are kept");

        for entry_id in [
            AppSettingsEntryId::SelectExperimentalDetailOptions,
            AppSettingsEntryId::ScanFollowSymlinks,
            AppSettingsEntryId::ScreenshotCopyToClipboard,
            AppSettingsEntryId::LoggingLevel,
        ] {
            let original = config.clone();
            let session =
                AppSettingsEditSession::capture(&config, entry_id, AppSettingsChoices::None);
            assert!(session.adjust(&mut config, 1), "{entry_id:?}");
            session.restore(&mut config);
            assert_eq!(
                toml::to_string(&config).unwrap(),
                toml::to_string(&original).unwrap(),
                "{entry_id:?}"
            );
        }
    }
}
