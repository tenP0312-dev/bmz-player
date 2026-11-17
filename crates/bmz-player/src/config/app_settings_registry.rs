use super::app_config::{
    AppConfig, AudioBackend, AudioBufferSizeMode, AudioOutputMode, AudioSampleRateMode,
    FrameLatencyModeConfig, GamepadBackendKind, InputBackendKind, InternalResolutionModeConfig,
    LogLevel, RendererBackend, VsyncModeConfig, WindowMode,
};
use super::download_sources::HttpDownloadPreset;
use crate::i18n::{AppLocale, Localizer};

const AUDIO_SAMPLE_RATES: [u32; 5] = [44_100, 48_000, 96_000, 192_000, 384_000];

/// 選曲画面から編集できる `data/config.toml` の項目。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppSettingsEntryId {
    AudioBackend,
    AudioOutputMode,
    AudioSampleRate,
    AudioBufferMode,
    AudioBufferSize,
    AudioOutputDevice,
    AudioAsioDriver,
    AudioOutputChannelPair,
    VideoWindowMode,
    VideoWidth,
    VideoHeight,
    VideoInternalResolution,
    VideoMonitor,
    VideoVsyncMode,
    VideoFrameLatencyMode,
    VideoTargetFps,
    VideoBackgroundFps,
    VideoRenderer,
    InputKeyboardBackend,
    InputGamepadBackend,
    InputKeyboardEnabled,
    SelectExperimentalDetailOptions,
    SelectScrollDurationLow,
    SelectScrollDurationHigh,
    ScanFollowSymlinks,
    ScanOnStartup,
    TablesFetchOnStartup,
    DownloadsIpfsEnabled,
    DownloadsHttpEnabled,
    DownloadsHttpSource,
    DownloadsHttpParallelism,
    DownloadsHttpKeepArchive,
    ScreenshotCopyToClipboard,
    LoggingLevel,
    LoggingFile,
}

/// The `config.toml` section an entry edits. Cancelling an edit restores only this section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSettingsSection {
    Audio,
    Video,
    Input,
    Select,
    Scan,
    Tables,
    Downloads,
    Screenshot,
    Logging,
}

impl AppSettingsEntryId {
    // Controller input ON / OFF is left to the BMZ menu: turning it off from a
    // controller would leave a controller-only player without input.
    pub const INPUT_ENTRIES: &'static [Self] =
        &[Self::InputKeyboardBackend, Self::InputGamepadBackend, Self::InputKeyboardEnabled];

    pub const SELECT_ENTRIES: &'static [Self] = &[
        Self::SelectExperimentalDetailOptions,
        Self::SelectScrollDurationLow,
        Self::SelectScrollDurationHigh,
    ];

    pub const SCAN_ENTRIES: &'static [Self] = &[Self::ScanFollowSymlinks, Self::ScanOnStartup];

    pub const TABLES_ENTRIES: &'static [Self] = &[Self::TablesFetchOnStartup];

    pub const DOWNLOADS_ENTRIES: &'static [Self] = &[
        Self::DownloadsIpfsEnabled,
        Self::DownloadsHttpEnabled,
        Self::DownloadsHttpSource,
        Self::DownloadsHttpParallelism,
        Self::DownloadsHttpKeepArchive,
    ];

    pub const SYSTEM_ENTRIES: &'static [Self] =
        &[Self::ScreenshotCopyToClipboard, Self::LoggingLevel, Self::LoggingFile];

    pub const fn section(self) -> AppSettingsSection {
        match self {
            Self::AudioBackend
            | Self::AudioOutputMode
            | Self::AudioSampleRate
            | Self::AudioBufferMode
            | Self::AudioBufferSize
            | Self::AudioOutputDevice
            | Self::AudioAsioDriver
            | Self::AudioOutputChannelPair => AppSettingsSection::Audio,
            Self::VideoWindowMode
            | Self::VideoWidth
            | Self::VideoHeight
            | Self::VideoInternalResolution
            | Self::VideoMonitor
            | Self::VideoVsyncMode
            | Self::VideoFrameLatencyMode
            | Self::VideoTargetFps
            | Self::VideoBackgroundFps
            | Self::VideoRenderer => AppSettingsSection::Video,
            Self::InputKeyboardBackend | Self::InputGamepadBackend | Self::InputKeyboardEnabled => {
                AppSettingsSection::Input
            }
            Self::SelectExperimentalDetailOptions
            | Self::SelectScrollDurationLow
            | Self::SelectScrollDurationHigh => AppSettingsSection::Select,
            Self::ScanFollowSymlinks | Self::ScanOnStartup => AppSettingsSection::Scan,
            Self::TablesFetchOnStartup => AppSettingsSection::Tables,
            Self::DownloadsIpfsEnabled
            | Self::DownloadsHttpEnabled
            | Self::DownloadsHttpSource
            | Self::DownloadsHttpParallelism
            | Self::DownloadsHttpKeepArchive => AppSettingsSection::Downloads,
            Self::ScreenshotCopyToClipboard => AppSettingsSection::Screenshot,
            Self::LoggingLevel | Self::LoggingFile => AppSettingsSection::Logging,
        }
    }

    pub const VIDEO_ENTRIES: &'static [Self] = &[
        Self::VideoWindowMode,
        Self::VideoWidth,
        Self::VideoHeight,
        Self::VideoInternalResolution,
        Self::VideoMonitor,
        Self::VideoVsyncMode,
        Self::VideoFrameLatencyMode,
        Self::VideoTargetFps,
        Self::VideoBackgroundFps,
        Self::VideoRenderer,
    ];

    pub const fn is_audio(self) -> bool {
        matches!(self.section(), AppSettingsSection::Audio)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::AudioBackend => "AUDIO BACKEND",
            Self::AudioOutputMode => "OUTPUT MODE",
            Self::AudioSampleRate => "SAMPLE RATE",
            Self::AudioBufferMode => "BUFFER MODE",
            Self::AudioBufferSize => "BUFFER SIZE",
            Self::AudioOutputDevice => "OUTPUT DEVICE",
            Self::AudioAsioDriver => "ASIO DRIVER",
            Self::AudioOutputChannelPair => "OUTPUT CHANNEL",
            Self::VideoWindowMode => "WINDOW MODE",
            Self::VideoWidth => "WINDOW WIDTH",
            Self::VideoHeight => "WINDOW HEIGHT",
            Self::VideoInternalResolution => "INTERNAL RESOLUTION",
            Self::VideoMonitor => "FULLSCREEN DISPLAY",
            Self::VideoVsyncMode => "VSYNC",
            Self::VideoFrameLatencyMode => "FRAME LATENCY",
            Self::VideoTargetFps => "TARGET FPS",
            Self::VideoBackgroundFps => "BACKGROUND FPS",
            Self::VideoRenderer => "RENDERER",
            Self::InputKeyboardBackend => "KEYBOARD BACKEND",
            Self::InputGamepadBackend => "CONTROLLER BACKEND",
            Self::InputKeyboardEnabled => "KEYBOARD INPUT",
            Self::SelectExperimentalDetailOptions => "DETAIL OPTIONS (EXPERIMENTAL)",
            Self::SelectScrollDurationLow => "SCROLL FIRST",
            Self::SelectScrollDurationHigh => "SCROLL REPEAT",
            Self::ScanFollowSymlinks => "FOLLOW SYMLINKS",
            Self::ScanOnStartup => "RESCAN ON STARTUP",
            Self::TablesFetchOnStartup => "FETCH TABLES ON STARTUP",
            Self::DownloadsIpfsEnabled => "IPFS DOWNLOAD",
            Self::DownloadsHttpEnabled => "HTTP DOWNLOAD",
            Self::DownloadsHttpSource => "HTTP SOURCE",
            Self::DownloadsHttpParallelism => "HTTP PARALLEL",
            Self::DownloadsHttpKeepArchive => "KEEP HTTP ARCHIVE",
            Self::ScreenshotCopyToClipboard => "SCREENSHOT CLIPBOARD",
            Self::LoggingLevel => "LOG LEVEL",
            Self::LoggingFile => "LOG FILE",
        }
    }

    pub const fn description_key(self) -> &'static str {
        match self {
            Self::AudioBackend => "settings-entry-description-audio-backend",
            Self::AudioOutputMode => "settings-entry-description-audio-output-mode",
            Self::AudioSampleRate => "settings-entry-description-audio-sample-rate",
            Self::AudioBufferMode => "settings-entry-description-audio-buffer-mode",
            Self::AudioBufferSize => "settings-entry-description-audio-buffer-size",
            Self::AudioOutputDevice => "settings-entry-description-audio-output-device",
            Self::AudioAsioDriver => "settings-entry-description-audio-asio-driver",
            Self::AudioOutputChannelPair => "settings-entry-description-audio-output-channel",
            Self::VideoWindowMode => "settings-entry-description-video-window-mode",
            Self::VideoWidth | Self::VideoHeight => "settings-entry-description-video-window-size",
            Self::VideoInternalResolution => "settings-entry-description-video-internal-resolution",
            Self::VideoMonitor => "settings-entry-description-video-monitor",
            Self::VideoVsyncMode => "settings-entry-description-video-vsync",
            Self::VideoFrameLatencyMode => "settings-entry-description-video-frame-latency",
            Self::VideoTargetFps => "settings-entry-description-video-target-fps",
            Self::VideoBackgroundFps => "settings-entry-description-video-background-fps",
            Self::VideoRenderer => "settings-entry-description-video-renderer",
            Self::InputKeyboardBackend => "settings-entry-description-input-keyboard-backend",
            Self::InputGamepadBackend => "settings-entry-description-input-gamepad-backend",
            Self::InputKeyboardEnabled => "settings-entry-description-input-keyboard-enabled",
            Self::SelectExperimentalDetailOptions => {
                "settings-select-experimental-detail-options-help"
            }
            Self::SelectScrollDurationLow => "settings-entry-description-select-scroll-first",
            Self::SelectScrollDurationHigh => "settings-entry-description-select-scroll-repeat",
            Self::ScanFollowSymlinks => "settings-entry-description-scan-follow-symlinks",
            Self::ScanOnStartup => "settings-entry-description-scan-on-startup",
            Self::TablesFetchOnStartup => "settings-entry-description-tables-fetch-on-startup",
            Self::DownloadsIpfsEnabled => "settings-entry-description-downloads-ipfs",
            Self::DownloadsHttpEnabled => "settings-entry-description-downloads-http",
            Self::DownloadsHttpSource => "settings-downloads-source-help",
            Self::DownloadsHttpParallelism => "settings-entry-description-downloads-parallelism",
            Self::DownloadsHttpKeepArchive => "settings-downloads-keep-archive-help",
            Self::ScreenshotCopyToClipboard => "settings-entry-description-screenshot-clipboard",
            Self::LoggingLevel | Self::LoggingFile => "settings-logging-help",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppSettingsChoices {
    None,
    Text(Vec<String>),
    AudioBackends(Vec<AudioBackend>),
    Renderers(Vec<RendererBackend>),
    KeyboardBackends(Vec<InputBackendKind>),
    GamepadBackends(Vec<GamepadBackendKind>),
}

pub fn format_app_settings_value(
    config: &AppConfig,
    entry_id: AppSettingsEntryId,
    locale: AppLocale,
) -> String {
    let text = Localizer::new(locale);
    match entry_id {
        AppSettingsEntryId::AudioBackend => audio_backend_label(&config.audio.backend, text),
        AppSettingsEntryId::AudioOutputMode => {
            audio_output_mode_label(&config.audio.output_mode, text)
        }
        AppSettingsEntryId::AudioSampleRate => {
            if config.audio.sample_rate_mode == AudioSampleRateMode::Auto {
                text.text("settings-audio-auto-driver-default")
            } else {
                audio_sample_rate_label(config.audio.sample_rate)
            }
        }
        AppSettingsEntryId::AudioBufferMode => match config.audio.buffer_size_mode {
            AudioBufferSizeMode::Auto => text.text("common-auto"),
            AudioBufferSizeMode::Fixed => text.text("common-fixed"),
        },
        AppSettingsEntryId::AudioBufferSize => format!("{} frames", config.audio.buffer_size),
        AppSettingsEntryId::AudioOutputDevice => {
            display_optional_name(&config.audio.output_device, text.text("common-default"))
        }
        AppSettingsEntryId::AudioAsioDriver => {
            display_optional_name(&config.audio.asio_driver, text.text("common-unspecified"))
        }
        AppSettingsEntryId::AudioOutputChannelPair => {
            audio_channel_pair_label(config.audio.output_channel_pair)
        }
        AppSettingsEntryId::VideoWindowMode => window_mode_label(&config.video.mode, text),
        AppSettingsEntryId::VideoWidth => format!("{} px", config.video.width),
        AppSettingsEntryId::VideoHeight => format!("{} px", config.video.height),
        AppSettingsEntryId::VideoInternalResolution => {
            internal_resolution_label(&config.video.internal_resolution, text)
        }
        AppSettingsEntryId::VideoMonitor => display_optional_name(
            &config.video.monitor_name,
            text.text("settings-video-primary-monitor"),
        ),
        AppSettingsEntryId::VideoVsyncMode => match config.video.vsync_mode {
            VsyncModeConfig::Vsync => "Vsync (Fifo)".to_string(),
            VsyncModeConfig::AdaptiveVsync => "Adaptive Vsync (Fifo Relaxed)".to_string(),
            VsyncModeConfig::VsyncOff => "Vsync Off (Immediate)".to_string(),
            VsyncModeConfig::FastVsync => "Fast Vsync (Mailbox)".to_string(),
        },
        AppSettingsEntryId::VideoFrameLatencyMode => {
            frame_latency_label(config.video.frame_latency_mode, text)
        }
        AppSettingsEntryId::VideoTargetFps => {
            if config.video.target_fps == 0 {
                "UNLIMITED".to_string()
            } else {
                format!("{} FPS", config.video.target_fps)
            }
        }
        AppSettingsEntryId::VideoBackgroundFps => {
            format!("{} FPS", config.video.frame_limit_in_background)
        }
        AppSettingsEntryId::VideoRenderer => renderer_label(&config.video.renderer, text),
        AppSettingsEntryId::InputKeyboardBackend => {
            keyboard_backend_label(&config.input.backend, text)
        }
        AppSettingsEntryId::InputGamepadBackend => {
            gamepad_backend_label(&config.input.gamepad_backend, text)
        }
        AppSettingsEntryId::InputKeyboardEnabled => on_off(config.input.keyboard_enabled),
        AppSettingsEntryId::SelectExperimentalDetailOptions => {
            on_off(config.select.experimental_detail_options)
        }
        AppSettingsEntryId::SelectScrollDurationLow => {
            format!("{} ms", config.select.scroll_duration_low_ms)
        }
        AppSettingsEntryId::SelectScrollDurationHigh => {
            format!("{} ms", config.select.scroll_duration_high_ms)
        }
        AppSettingsEntryId::ScanFollowSymlinks => on_off(config.scan.follow_symlinks),
        AppSettingsEntryId::ScanOnStartup => on_off(config.scan.auto_rescan_on_startup),
        AppSettingsEntryId::TablesFetchOnStartup => on_off(config.tables.auto_fetch_on_startup),
        AppSettingsEntryId::DownloadsIpfsEnabled => on_off(config.downloads.ipfs_enabled),
        AppSettingsEntryId::DownloadsHttpEnabled => on_off(config.downloads.http_enabled),
        AppSettingsEntryId::DownloadsHttpSource => config.downloads.http_source.name().to_string(),
        AppSettingsEntryId::DownloadsHttpParallelism => {
            config.downloads.http_parallelism.to_string()
        }
        AppSettingsEntryId::DownloadsHttpKeepArchive => on_off(config.downloads.http_keep_archive),
        AppSettingsEntryId::ScreenshotCopyToClipboard => {
            on_off(config.screenshot.copy_to_clipboard)
        }
        AppSettingsEntryId::LoggingLevel => log_level_label(config.logging.level).to_string(),
        AppSettingsEntryId::LoggingFile => on_off(config.logging.file_logging),
    }
}

pub fn adjust_app_settings_value(
    config: &mut AppConfig,
    entry_id: AppSettingsEntryId,
    choices: &AppSettingsChoices,
    direction: i32,
) -> bool {
    if direction == 0 {
        return false;
    }
    let before = format_app_settings_value(config, entry_id, AppLocale::DEFAULT);
    match entry_id {
        AppSettingsEntryId::AudioBackend => {
            if let AppSettingsChoices::AudioBackends(values) = choices {
                cycle_value(&mut config.audio.backend, values, direction);
            }
        }
        AppSettingsEntryId::AudioOutputMode => cycle_value(
            &mut config.audio.output_mode,
            &[
                AudioOutputMode::Shared,
                AudioOutputMode::SharedLowLatency,
                AudioOutputMode::Exclusive,
            ],
            direction,
        ),
        AppSettingsEntryId::AudioSampleRate => {
            let current = if config.audio.sample_rate_mode == AudioSampleRateMode::Auto {
                0
            } else {
                AUDIO_SAMPLE_RATES
                    .iter()
                    .position(|rate| *rate == config.audio.sample_rate)
                    .map(|index| index + 1)
                    .unwrap_or(0)
            };
            let next = cycle_index(current, AUDIO_SAMPLE_RATES.len() + 1, direction);
            if next == 0 {
                config.audio.sample_rate_mode = AudioSampleRateMode::Auto;
            } else {
                config.audio.sample_rate_mode = AudioSampleRateMode::Fixed;
                config.audio.sample_rate = AUDIO_SAMPLE_RATES[next - 1];
            }
        }
        AppSettingsEntryId::AudioBufferMode => cycle_value(
            &mut config.audio.buffer_size_mode,
            &[AudioBufferSizeMode::Auto, AudioBufferSizeMode::Fixed],
            direction,
        ),
        AppSettingsEntryId::AudioBufferSize => {
            config.audio.buffer_size =
                adjust_bounded(config.audio.buffer_size, direction, 16, 16, 4096);
        }
        AppSettingsEntryId::AudioOutputDevice => {
            if let AppSettingsChoices::Text(values) = choices {
                cycle_value(&mut config.audio.output_device, values, direction);
            }
        }
        AppSettingsEntryId::AudioAsioDriver => {
            if let AppSettingsChoices::Text(values) = choices {
                cycle_value(&mut config.audio.asio_driver, values, direction);
            }
        }
        AppSettingsEntryId::AudioOutputChannelPair => {
            config.audio.output_channel_pair =
                cycle_index(config.audio.output_channel_pair as usize, 6, direction) as u32;
        }
        AppSettingsEntryId::VideoWindowMode => cycle_value(
            &mut config.video.mode,
            &[
                WindowMode::Windowed,
                WindowMode::BorderlessFullscreen,
                WindowMode::ExclusiveFullscreen,
            ],
            direction,
        ),
        AppSettingsEntryId::VideoWidth => {
            config.video.width = adjust_bounded(config.video.width, direction, 10, 640, 3840);
        }
        AppSettingsEntryId::VideoHeight => {
            config.video.height = adjust_bounded(config.video.height, direction, 10, 480, 2160);
        }
        AppSettingsEntryId::VideoInternalResolution => cycle_value(
            &mut config.video.internal_resolution,
            &[InternalResolutionModeConfig::Native, InternalResolutionModeConfig::Skin],
            direction,
        ),
        AppSettingsEntryId::VideoMonitor => {
            if let AppSettingsChoices::Text(values) = choices {
                cycle_value(&mut config.video.monitor_name, values, direction);
            }
        }
        AppSettingsEntryId::VideoVsyncMode => cycle_value(
            &mut config.video.vsync_mode,
            &[
                VsyncModeConfig::Vsync,
                VsyncModeConfig::AdaptiveVsync,
                VsyncModeConfig::VsyncOff,
                VsyncModeConfig::FastVsync,
            ],
            direction,
        ),
        AppSettingsEntryId::VideoFrameLatencyMode => cycle_value(
            &mut config.video.frame_latency_mode,
            &[
                FrameLatencyModeConfig::Auto,
                FrameLatencyModeConfig::LowLatency,
                FrameLatencyModeConfig::Stable,
            ],
            direction,
        ),
        AppSettingsEntryId::VideoTargetFps => {
            config.video.target_fps = adjust_unbounded(config.video.target_fps, direction);
        }
        AppSettingsEntryId::VideoBackgroundFps => {
            config.video.frame_limit_in_background =
                adjust_bounded(config.video.frame_limit_in_background, direction, 1, 1, 120);
        }
        AppSettingsEntryId::VideoRenderer => {
            if let AppSettingsChoices::Renderers(values) = choices {
                cycle_value(&mut config.video.renderer, values, direction);
            }
        }
        AppSettingsEntryId::InputKeyboardBackend => {
            if let AppSettingsChoices::KeyboardBackends(values) = choices {
                cycle_value(&mut config.input.backend, values, direction);
            }
        }
        AppSettingsEntryId::InputGamepadBackend => {
            if let AppSettingsChoices::GamepadBackends(values) = choices {
                cycle_value(&mut config.input.gamepad_backend, values, direction);
            }
        }
        AppSettingsEntryId::InputKeyboardEnabled => {
            config.input.keyboard_enabled = !config.input.keyboard_enabled;
        }
        AppSettingsEntryId::SelectExperimentalDetailOptions => {
            config.select.experimental_detail_options = !config.select.experimental_detail_options;
        }
        // Same ranges as the BMZ menu sliders.
        AppSettingsEntryId::SelectScrollDurationLow => {
            config.select.scroll_duration_low_ms =
                adjust_bounded(config.select.scroll_duration_low_ms, direction, 10, 2, 1000);
        }
        AppSettingsEntryId::SelectScrollDurationHigh => {
            config.select.scroll_duration_high_ms =
                adjust_bounded(config.select.scroll_duration_high_ms, direction, 10, 1, 1000);
        }
        AppSettingsEntryId::ScanFollowSymlinks => {
            config.scan.follow_symlinks = !config.scan.follow_symlinks;
        }
        AppSettingsEntryId::ScanOnStartup => {
            config.scan.auto_rescan_on_startup = !config.scan.auto_rescan_on_startup;
        }
        AppSettingsEntryId::TablesFetchOnStartup => {
            config.tables.auto_fetch_on_startup = !config.tables.auto_fetch_on_startup;
        }
        AppSettingsEntryId::DownloadsIpfsEnabled => {
            config.downloads.ipfs_enabled = !config.downloads.ipfs_enabled;
        }
        AppSettingsEntryId::DownloadsHttpEnabled => {
            config.downloads.http_enabled = !config.downloads.http_enabled;
        }
        AppSettingsEntryId::DownloadsHttpSource => {
            let presets: Vec<_> = HttpDownloadPreset::ALL
                .into_iter()
                .filter(|preset| preset.is_available())
                .collect();
            let before = config.downloads.http_source;
            cycle_value(&mut config.downloads.http_source, &presets, direction);
            // Same as the BMZ menu: only a preset replaces the custom URL.
            if config.downloads.http_source != before
                && config.downloads.http_source != HttpDownloadPreset::Custom
            {
                config.downloads.http_api_url = config.downloads.http_source.api_url().into();
            }
        }
        AppSettingsEntryId::DownloadsHttpParallelism => {
            let value =
                adjust_bounded(u32::from(config.downloads.http_parallelism), direction, 1, 1, 5);
            config.downloads.http_parallelism = value as u8;
        }
        AppSettingsEntryId::DownloadsHttpKeepArchive => {
            config.downloads.http_keep_archive = !config.downloads.http_keep_archive;
        }
        AppSettingsEntryId::ScreenshotCopyToClipboard => {
            config.screenshot.copy_to_clipboard = !config.screenshot.copy_to_clipboard;
        }
        AppSettingsEntryId::LoggingLevel => cycle_value(
            &mut config.logging.level,
            &[LogLevel::Trace, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error],
            direction,
        ),
        AppSettingsEntryId::LoggingFile => {
            config.logging.file_logging = !config.logging.file_logging;
        }
    }
    before != format_app_settings_value(config, entry_id, AppLocale::DEFAULT)
}

fn adjust_bounded(value: u32, direction: i32, step: u32, min: u32, max: u32) -> u32 {
    let delta = i64::from(direction) * i64::from(step);
    (i64::from(value) + delta).clamp(i64::from(min), i64::from(max)) as u32
}

fn adjust_unbounded(value: u32, direction: i32) -> u32 {
    if direction > 0 {
        value.saturating_add(direction as u32)
    } else {
        value.saturating_sub(direction.unsigned_abs())
    }
}

fn cycle_value<T: Clone + PartialEq>(value: &mut T, values: &[T], direction: i32) {
    if values.is_empty() {
        return;
    }
    let current = values
        .iter()
        .position(|candidate| candidate == value)
        .unwrap_or_else(|| if direction > 0 { values.len() - 1 } else { 0 });
    *value = values[cycle_index(current, values.len(), direction)].clone();
}

fn cycle_index(current: usize, len: usize, direction: i32) -> usize {
    (current as i64 + i64::from(direction)).rem_euclid(len as i64) as usize
}

fn on_off(value: bool) -> String {
    if value { "ON" } else { "OFF" }.to_string()
}

fn keyboard_backend_label(backend: &InputBackendKind, text: Localizer) -> String {
    match backend {
        InputBackendKind::Winit => "winit".to_string(),
        InputBackendKind::LinuxEvdev => "Linux evdev (X11)".to_string(),
        InputBackendKind::MacOsHid => "macOS IOHID".to_string(),
        InputBackendKind::MacOsGameController => "GCKeyboard".to_string(),
        InputBackendKind::RawInput => text.text("settings-input-raw-input"),
        // Hid / Midi are legacy values migrated to Auto when loaded.
        InputBackendKind::Auto | InputBackendKind::Hid | InputBackendKind::Midi => {
            text.text("common-auto-select")
        }
    }
}

fn gamepad_backend_label(backend: &GamepadBackendKind, text: Localizer) -> String {
    match backend {
        GamepadBackendKind::Auto => text.text("common-auto-select"),
        GamepadBackendKind::Gilrs => "gilrs".to_string(),
        GamepadBackendKind::RawInput => text.text("settings-input-raw-input"),
        GamepadBackendKind::GameInput => text.text("settings-input-gameinput"),
        GamepadBackendKind::GameController => "GameController".to_string(),
    }
}

fn log_level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
}

fn display_optional_name(value: &str, fallback: String) -> String {
    if value.trim().is_empty() { fallback } else { value.to_string() }
}

fn audio_backend_label(backend: &AudioBackend, text: Localizer) -> String {
    match backend {
        AudioBackend::Auto => text.text("common-auto-select"),
        AudioBackend::Wasapi => "WASAPI".to_string(),
        AudioBackend::Asio => "ASIO".to_string(),
        AudioBackend::CoreAudio => "Core Audio".to_string(),
        AudioBackend::CoreAudioIoProc => "Core Audio IOProc".to_string(),
        AudioBackend::Alsa => "ALSA".to_string(),
        AudioBackend::Pulse => "PulseAudio".to_string(),
        AudioBackend::PipeWire => "PipeWire".to_string(),
    }
}

fn audio_output_mode_label(mode: &AudioOutputMode, text: Localizer) -> String {
    match mode {
        AudioOutputMode::Shared => text.text("settings-audio-output-mode-shared"),
        AudioOutputMode::SharedLowLatency => text.text("settings-audio-output-mode-low-latency"),
        AudioOutputMode::Exclusive => text.text("settings-audio-output-mode-exclusive"),
    }
}

fn audio_sample_rate_label(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{}kHz", hz / 1000)
    } else {
        format!("{:.1}kHz", hz as f64 / 1000.0)
    }
}

fn audio_channel_pair_label(pair: u32) -> String {
    let left = pair * 2 + 1;
    format!("{}-{}ch", left, left + 1)
}

fn window_mode_label(mode: &WindowMode, text: Localizer) -> String {
    match mode {
        WindowMode::Windowed => text.text("settings-windowed"),
        WindowMode::BorderlessFullscreen => text.text("settings-borderless-fullscreen"),
        WindowMode::ExclusiveFullscreen => text.text("settings-exclusive-fullscreen"),
    }
}

fn internal_resolution_label(mode: &InternalResolutionModeConfig, text: Localizer) -> String {
    match mode {
        InternalResolutionModeConfig::Native => {
            text.text("settings-video-internal-resolution-native")
        }
        InternalResolutionModeConfig::Skin => text.text("settings-video-internal-resolution-skin"),
    }
}

fn frame_latency_label(mode: FrameLatencyModeConfig, text: Localizer) -> String {
    match mode {
        FrameLatencyModeConfig::Auto => text.text("settings-video-frame-latency-auto"),
        FrameLatencyModeConfig::LowLatency => text.text("settings-video-frame-latency-low-latency"),
        FrameLatencyModeConfig::Stable => text.text("settings-video-frame-latency-stable"),
    }
}

fn renderer_label(backend: &RendererBackend, text: Localizer) -> String {
    match backend {
        RendererBackend::Auto => text.text("common-auto-select"),
        RendererBackend::Vulkan => "Vulkan".to_string(),
        RendererBackend::Metal => "Metal".to_string(),
        RendererBackend::Dx12 => "DirectX 12".to_string(),
        RendererBackend::Gl => "OpenGL".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_buffer_adjustment_reaches_and_stops_at_16_frames() {
        let mut config = AppConfig::default();
        config.audio.buffer_size = 32;
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::AudioBufferSize,
            &AppSettingsChoices::None,
            -1
        ));
        assert_eq!(config.audio.buffer_size, 16);
        assert!(!adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::AudioBufferSize,
            &AppSettingsChoices::None,
            -1
        ));
    }

    #[test]
    fn sample_rate_cycles_between_auto_and_fixed_presets() {
        let mut config = AppConfig::default();
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::AudioSampleRate,
            &AppSettingsChoices::None,
            1,
        ));
        assert_eq!(config.audio.sample_rate_mode, AudioSampleRateMode::Fixed);
        assert_eq!(config.audio.sample_rate, 44_100);

        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::AudioSampleRate,
            &AppSettingsChoices::None,
            -1,
        ));
        assert_eq!(config.audio.sample_rate_mode, AudioSampleRateMode::Auto);
    }

    #[test]
    fn dynamic_text_choices_keep_the_default_choice() {
        let mut config = AppConfig::default();
        let choices = AppSettingsChoices::Text(vec![String::new(), "Speakers".to_string()]);
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::AudioOutputDevice,
            &choices,
            1,
        ));
        assert_eq!(config.audio.output_device, "Speakers");
    }

    #[test]
    fn video_numeric_values_use_ui_bounds() {
        let mut config = AppConfig::default();
        config.video.width = 3840;
        assert!(!adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::VideoWidth,
            &AppSettingsChoices::None,
            1,
        ));
        config.video.frame_limit_in_background = 1;
        assert!(!adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::VideoBackgroundFps,
            &AppSettingsChoices::None,
            -1,
        ));
    }

    #[test]
    fn http_source_preset_sets_the_url_but_custom_keeps_it() {
        let mut config = AppConfig::default();
        config.downloads.http_source = HttpDownloadPreset::Custom;
        config.downloads.http_api_url = "https://mine.example/{md5}".into();
        let none = AppSettingsChoices::None;
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::DownloadsHttpSource,
            &none,
            1
        ));
        assert_ne!(config.downloads.http_source, HttpDownloadPreset::Custom);
        assert!(config.downloads.http_source.is_available());
        assert_eq!(config.downloads.http_api_url, config.downloads.http_source.api_url());
        let mut previous_url = config.downloads.http_api_url.clone();
        while config.downloads.http_source != HttpDownloadPreset::Custom {
            previous_url = config.downloads.http_api_url.clone();
            adjust_app_settings_value(
                &mut config,
                AppSettingsEntryId::DownloadsHttpSource,
                &none,
                1,
            );
        }
        assert_eq!(config.downloads.http_api_url, previous_url, "Custom keeps the last URL");
    }

    #[test]
    fn new_numeric_entries_use_the_menu_ranges() {
        let mut config = AppConfig::default();
        let none = AppSettingsChoices::None;
        config.downloads.http_parallelism = 5;
        assert!(!adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::DownloadsHttpParallelism,
            &none,
            1
        ));
        config.select.scroll_duration_low_ms = 5;
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::SelectScrollDurationLow,
            &none,
            -1
        ));
        assert_eq!(config.select.scroll_duration_low_ms, 2);
        config.select.scroll_duration_high_ms = 995;
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::SelectScrollDurationHigh,
            &none,
            1
        ));
        assert_eq!(config.select.scroll_duration_high_ms, 1000);
    }

    #[test]
    fn backends_cycle_only_through_offered_choices() {
        let mut config = AppConfig::default();
        config.input.backend = InputBackendKind::Winit;
        let choices = AppSettingsChoices::KeyboardBackends(vec![
            InputBackendKind::Auto,
            InputBackendKind::Winit,
        ]);
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::InputKeyboardBackend,
            &choices,
            1
        ));
        assert_eq!(config.input.backend, InputBackendKind::Auto);
        // Without the list of available backends nothing changes.
        assert!(!adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::InputGamepadBackend,
            &AppSettingsChoices::None,
            1
        ));
        config.logging.level = LogLevel::Error;
        assert!(adjust_app_settings_value(
            &mut config,
            AppSettingsEntryId::LoggingLevel,
            &AppSettingsChoices::None,
            1
        ));
        assert_eq!(config.logging.level, LogLevel::Trace);
    }
}
