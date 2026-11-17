//! Settings folder rows that are not single profile / app config values: lists
//! built from the current state (profiles, skins, song folders, tables,
//! controllers) and one-shot operations. Each change goes through the same path
//! as the BMZ menu.

use super::*;
use crate::config::app_settings_registry::AppSettingsSection;
use crate::screens::settings_edit::ChoiceEditSession;
use crate::screens::settings_model::{
    KeyDefaultsScope, SettingsActionKind, SettingsChoice, SettingsChoiceTarget,
    SettingsFolderExtras, SkinFolderData, in_settings_stack, settings_skin_target,
};
use crate::ui::{Lr2SkinTarget, ProfileManagerAction, lr2_set_skin_item, lr2_skin_item_choices};

impl WinitApp {
    /// Confirm on a list or operation row of the settings folder.
    pub(super) fn activate_selected_settings_row(&mut self) {
        let index = self.select.selected_index;
        match self.select.select_items.get(index).cloned() {
            Some(SelectItem::SettingsChoice(row)) => {
                if row.choices.is_empty() {
                    return;
                }
                tracing::info!(target = ?row.target, "settings choice edit started");
                self.select.settings_edit =
                    Some(SelectSettingsEditSession::Choice(ChoiceEditSession::begin(index, row)));
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            Some(SelectItem::SettingsAction(row)) => self.run_settings_action(row.kind),
            _ => {}
        }
    }

    pub(super) fn adjust_settings_choice_edit(&mut self, direction: i32) {
        let Some(SelectSettingsEditSession::Choice(session)) = self.select.settings_edit.as_mut()
        else {
            return;
        };
        if !session.adjust(direction) {
            return;
        }
        let (index, target, current) =
            (session.item_index, session.row.target, session.row.current);
        self.show_settings_choice_draft(index, target, current);
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    /// The row in the list shows the draft value; the edit session keeps its own copy.
    fn show_settings_choice_draft(
        &mut self,
        index: usize,
        target: SettingsChoiceTarget,
        current: usize,
    ) {
        if let Some(SelectItem::SettingsChoice(row)) = self.select.select_items.get_mut(index)
            && row.target == target
        {
            row.current = current;
        }
    }

    pub(super) fn cancel_settings_choice_edit(&mut self, session: ChoiceEditSession) {
        self.show_settings_choice_draft(session.item_index, session.row.target, session.original());
        tracing::info!(target = ?session.row.target, "settings choice edit cancelled");
    }

    pub(super) fn commit_settings_choice_edit(&mut self, session: ChoiceEditSession) {
        let target = session.row.target;
        let Some(choice) = session.changed_choice().cloned() else {
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            return;
        };
        if self.apply_settings_choice(target, &choice) {
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            tracing::info!(?target, value = %choice.key, "settings choice applied");
        } else {
            self.show_settings_choice_draft(session.item_index, target, session.original());
        }
        self.reload_select_items();
    }

    /// Applies a confirmed choice. Returns false when nothing changed or saving failed.
    pub(super) fn apply_settings_choice(
        &mut self,
        target: SettingsChoiceTarget,
        choice: &SettingsChoice,
    ) -> bool {
        if target == SettingsChoiceTarget::Profile && !self.profile_change_allowed() {
            let text = Localizer::new(self.boot.profile_config.ui.locale());
            self.show_left_overlay_toast(text.text("profile-manager-unavailable"));
            return false;
        }
        let before_input = self.boot.app_config.input.clone();
        let effect = apply_settings_choice_to_config(
            &mut self.boot.app_config,
            &mut self.boot.profile_config,
            &self.boot.app_paths,
            target,
            choice,
        );
        match effect {
            SettingsChoiceEffect::Unchanged => false,
            // The same request as the BMZ menu's profile switch button.
            SettingsChoiceEffect::SwitchProfile(action) => {
                self.request_profile_change(action);
                true
            }
            SettingsChoiceEffect::Skin(skin_target) => {
                self.finish_settings_skin_change(skin_target)
            }
            SettingsChoiceEffect::Library => self.save_library_settings_from_folder(),
            SettingsChoiceEffect::Input => self.save_input_settings_from_folder(before_input),
        }
    }

    /// Saves the profile and reloads the edited skin, like the BMZ menu's skin page.
    fn finish_settings_skin_change(&mut self, target: Lr2SkinTarget) -> bool {
        self.boot.profile_config.updated_at = now_unix_seconds();
        if let Err(error) = self.save_profile_config_now() {
            tracing::error!(%error, "failed to save skin settings");
        }
        self.reload_skins(target.reload_request());
        true
    }

    /// Song folders and tables: saved and published like the BMZ menu's library page.
    fn save_library_settings_from_folder(&mut self) -> bool {
        if let Err(error) = self.save_app_config_now() {
            tracing::error!(%error, "failed to save library settings");
            return false;
        }
        if let Err(error) = self.publish_song_scope() {
            tracing::error!(%error, "failed to update library song scope");
        }
        true
    }

    fn save_input_settings_from_folder(&mut self, before: GlobalInputConfig) -> bool {
        if let Err(error) = self.save_app_config_now() {
            tracing::error!(%error, "failed to save input settings");
            self.boot.app_config.input = before;
            return false;
        }
        if let Some(window) = self.window.clone() {
            self.apply_egui_input_config(&window, &before);
        }
        true
    }

    pub(super) fn run_settings_action(&mut self, kind: SettingsActionKind) {
        tracing::info!(?kind, "settings action");
        match kind {
            SettingsActionKind::RescanSongs => self.load_songs_and_reload(false),
            SettingsActionKind::FullRescanSongs => self.load_songs_and_reload(true),
            SettingsActionKind::FetchEnabledTables => {
                let urls = self
                    .boot
                    .app_config
                    .tables
                    .sources
                    .iter()
                    .filter(|source| source.enabled)
                    .map(|source| source.url.clone())
                    .collect();
                self.spawn_table_fetches(urls, "settings folder table fetch".to_string());
            }
            SettingsActionKind::FetchTable(index) => {
                let Some(source) = self.boot.app_config.tables.sources.get(index) else {
                    return;
                };
                let urls = vec![source.url.clone()];
                self.spawn_table_fetches(urls, "settings folder table fetch".to_string());
            }
            SettingsActionKind::GamepadAutoAssign
            | SettingsActionKind::GamepadSwap
            | SettingsActionKind::GamepadClearAssignment => {
                let connected: Vec<String> = self
                    .gamepad
                    .as_ref()
                    .map(|gamepad| gamepad.connected_gamepads())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|pad| pad.is_connected)
                    .map(|pad| pad.stable_id)
                    .collect();
                let before = self.boot.app_config.input.clone();
                apply_gamepad_assignment_action(&mut self.boot.app_config.input, kind, &connected);
                if !self.save_input_settings_from_folder(before) {
                    return;
                }
                self.reload_select_items();
            }
            SettingsActionKind::RestoreKeyDefaults { scope, slot } => {
                let section = match scope {
                    KeyDefaultsScope::Group(group) => match group {
                        crate::config::key_config::KeyBindingGroup::Common => {
                            EguiKeyConfigSection::Common
                        }
                        crate::config::key_config::KeyBindingGroup::Select => {
                            EguiKeyConfigSection::Select
                        }
                        crate::config::key_config::KeyBindingGroup::Play => {
                            EguiKeyConfigSection::Play
                        }
                        crate::config::key_config::KeyBindingGroup::Result => {
                            EguiKeyConfigSection::Result
                        }
                    },
                    KeyDefaultsScope::Mode(key_mode) => EguiKeyConfigSection::KeyMode(key_mode),
                };
                // The same action as the BMZ menu's "restore defaults" button; it saves
                // the profile and plays the option sound.
                self.apply_egui_key_config_action(EguiKeyConfigAction::RestoreDefaults {
                    section,
                    slot,
                });
                self.reload_select_items();
                return;
            }
        }
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    /// Effects of a confirmed `config.toml` edit beyond saving it.
    pub(super) fn apply_committed_app_setting(&mut self, session: &AppSettingsEditSession) {
        match session.entry_id.section() {
            AppSettingsSection::Video => {
                if let Some(window) = self.window.clone() {
                    self.apply_egui_video_config(&window);
                }
            }
            AppSettingsSection::Input => {
                if let (Some(before), Some(window)) =
                    (session.input_baseline().cloned(), self.window.clone())
                {
                    self.apply_egui_input_config(&window, &before);
                }
            }
            AppSettingsSection::Select => {
                // Same as a change from the BMZ menu.
                if session.select_baseline().is_some_and(|before| {
                    before.experimental_detail_options
                        != self.boot.app_config.select.experimental_detail_options
                }) {
                    self.cancel_select_option_session();
                    self.reload_skins(SkinReloadRequest { select: true, ..Default::default() });
                }
            }
            AppSettingsSection::Audio
            | AppSettingsSection::Scan
            | AppSettingsSection::Tables
            | AppSettingsSection::Downloads
            | AppSettingsSection::Screenshot
            | AppSettingsSection::Logging => {}
        }
    }

    /// Lists the settings folder rows need, for the folder the list action shows.
    pub(in crate::app) fn settings_folder_extras(
        &self,
        action: &SelectListAction,
    ) -> SettingsFolderExtras {
        let path = match action {
            SelectListAction::Enter(path) => Some(path.as_str()),
            _ => self.select.folder_stack.last().map(String::as_str),
        };
        let entering_settings = path.is_some_and(|path| in_settings_stack(&[path.to_string()]));
        if !entering_settings && !in_settings_stack(&self.select.folder_stack) {
            return SettingsFolderExtras::default();
        }
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let profiles = crate::profile_cmd::profile_summaries(&self.boot.app_paths)
            .map(|profiles| {
                profiles.into_iter().map(|summary| (summary.id, summary.display_name)).collect()
            })
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to list profiles for the settings folder");
                Vec::new()
            });
        let gamepads = self
            .gamepad
            .as_ref()
            .map(|gamepad| gamepad.connected_gamepads())
            .unwrap_or_default()
            .into_iter()
            .map(|pad| (pad.stable_id, format!("#{} {}", pad.backend_id, pad.name)))
            .collect();
        let table_names = self
            .select
            .difficulty_tables
            .iter()
            .map(|table| (table.source_url.clone(), table.name.clone()))
            .collect();
        let skin = path.and_then(settings_skin_target).map(|target| {
            settings_skin_folder_data(
                target,
                &self.boot.profile_config.skin,
                self.skin.skin_catalog.get(),
                &self.boot.app_paths,
                text,
            )
        });
        SettingsFolderExtras { profiles, gamepads, table_names, skin }
    }
}

/// What the app does after [`apply_settings_choice_to_config`].
#[derive(Debug, Clone)]
pub(super) enum SettingsChoiceEffect {
    Unchanged,
    /// Nothing is changed yet; the app requests the switch.
    SwitchProfile(ProfileManagerAction),
    /// The profile's skin settings changed: save and reload this slot.
    Skin(Lr2SkinTarget),
    /// Song folders or tables changed: save `config.toml` and publish the song scope.
    Library,
    /// Controller assignment changed: save `config.toml` and apply the input settings.
    Input,
}

/// Writes a confirmed choice into the configs, the same fields the BMZ menu
/// edits, and says what the app must do next.
pub(super) fn apply_settings_choice_to_config(
    app_config: &mut AppConfig,
    profile: &mut ProfileConfig,
    app_paths: &crate::paths::AppPaths,
    target: SettingsChoiceTarget,
    choice: &SettingsChoice,
) -> SettingsChoiceEffect {
    let enabled = choice.key == "on";
    match target {
        SettingsChoiceTarget::Profile => {
            if choice.key.is_empty() || choice.key == profile.id {
                return SettingsChoiceEffect::Unchanged;
            }
            SettingsChoiceEffect::SwitchProfile(ProfileManagerAction::Switch(choice.key.clone()))
        }
        SettingsChoiceTarget::SkinPath(skin_target) => {
            if skin_target.set_skin(&mut profile.skin, &choice.key) {
                SettingsChoiceEffect::Skin(skin_target)
            } else {
                SettingsChoiceEffect::Unchanged
            }
        }
        SettingsChoiceTarget::SkinItem(skin_target, index) => {
            let defs = skin_defs_from_path(
                app_paths,
                &profile.skin,
                skin_target.skin_path(&profile.skin),
                skin_target.is_play(),
            );
            if lr2_set_skin_item(skin_target, &defs, &mut profile.skin, index, &choice.key) {
                SettingsChoiceEffect::Skin(skin_target)
            } else {
                SettingsChoiceEffect::Unchanged
            }
        }
        SettingsChoiceTarget::SongRootEnabled(index) => {
            set_flag(app_config.songs.roots.get_mut(index).map(|root| &mut root.enabled), enabled)
        }
        SettingsChoiceTarget::SongRootRecursive(index) => {
            set_flag(app_config.songs.roots.get_mut(index).map(|root| &mut root.recursive), enabled)
        }
        SettingsChoiceTarget::TableEnabled(index) => set_flag(
            app_config.tables.sources.get_mut(index).map(|source| &mut source.enabled),
            enabled,
        ),
        SettingsChoiceTarget::GamepadSlot(slot) if slot < 2 => {
            let input = &mut app_config.input;
            let device = (!choice.key.is_empty()).then(|| choice.key.clone());
            if input.gamepad_slot_device_ids[slot] == device {
                return SettingsChoiceEffect::Unchanged;
            }
            input.gamepad_slot_device_ids[slot] = device;
            input.gamepad_slot_gilrs_ids[slot] = None;
            SettingsChoiceEffect::Input
        }
        SettingsChoiceTarget::GamepadSlot(_) => SettingsChoiceEffect::Unchanged,
    }
}

fn set_flag(flag: Option<&mut bool>, value: bool) -> SettingsChoiceEffect {
    match flag {
        Some(flag) if *flag != value => {
            *flag = value;
            SettingsChoiceEffect::Library
        }
        _ => SettingsChoiceEffect::Unchanged,
    }
}

/// Same changes as the BMZ menu's controller assignment buttons.
pub(super) fn apply_gamepad_assignment_action(
    input: &mut GlobalInputConfig,
    kind: SettingsActionKind,
    connected: &[String],
) {
    match kind {
        SettingsActionKind::GamepadAutoAssign => {
            input.gamepad_slot_device_ids[0] = connected.first().cloned();
            input.gamepad_slot_device_ids[1] = connected.get(1).cloned();
            input.gamepad_slot_gilrs_ids = [None, None];
        }
        SettingsActionKind::GamepadSwap => {
            input.gamepad_slot_device_ids.swap(0, 1);
            input.gamepad_slot_gilrs_ids.swap(0, 1);
        }
        SettingsActionKind::GamepadClearAssignment => {
            input.gamepad_slot_device_ids = [None, None];
            input.gamepad_slot_gilrs_ids = [None, None];
        }
        _ => {}
    }
}

pub(super) fn settings_skin_folder_data(
    target: Lr2SkinTarget,
    skin: &SkinConfig,
    catalog: &crate::ui::SkinCatalog,
    app_paths: &crate::paths::AppPaths,
    text: Localizer,
) -> SkinFolderData {
    let mut skins: Vec<SettingsChoice> = target
        .skin_choices(catalog, text)
        .into_iter()
        .map(|(path, name)| SettingsChoice::new(path, name))
        .collect();
    let current_path = target.skin_path(skin);
    if !skins.iter().any(|choice| choice.key == current_path) {
        // A path typed in the BMZ menu stays selectable.
        skins.push(SettingsChoice::new(current_path, current_path));
    }
    let current_skin = skins.iter().position(|choice| choice.key == current_path).unwrap_or(0);
    let defs = skin_defs_from_path(app_paths, skin, current_path, target.is_play());
    let items = lr2_skin_item_choices(target, &defs, skin, app_paths)
        .into_iter()
        .map(|item| {
            let choices = item
                .choices
                .into_iter()
                .map(|(value, label)| SettingsChoice::new(value, label))
                .collect();
            (item.name, choices, item.current)
        })
        .collect();
    SkinFolderData { target, skins, current_skin, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::profile_tests::ProfileTestDir;
    use crate::config::app_config::{DifficultyTableSource, PathEntry};
    use crate::screens::settings_model::{SettingsChoiceRow, load_settings_items_for_config};
    use crate::ui::{SkinCandidate, SkinCandidateOrigin};

    fn choice_row(items: &[SelectItem], target: SettingsChoiceTarget) -> SettingsChoiceRow {
        items
            .iter()
            .find_map(|item| match item {
                SelectItem::SettingsChoice(row) if row.target == target => Some(row.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("row {target:?} is listed"))
    }

    fn choose(row: &SettingsChoiceRow, key: &str) -> SettingsChoice {
        let mut session = ChoiceEditSession::begin(0, row.clone());
        for _ in 0..row.choices.len() {
            if session.row.choices[session.row.current].key == key {
                break;
            }
            assert!(session.adjust(1));
        }
        session.changed_choice().cloned().expect("a different value is chosen")
    }

    #[test]
    fn profile_choice_requests_the_menu_switch_without_changing_the_current_profile() {
        let data = ProfileTestDir::new();
        let mut boot = data.boot();
        crate::profile_cmd::create_profile(&data.paths, "other", Some("Other"), false).unwrap();
        let extras = SettingsFolderExtras {
            profiles: crate::profile_cmd::profile_summaries(&data.paths)
                .unwrap()
                .into_iter()
                .map(|summary| (summary.id, summary.display_name))
                .collect(),
            ..Default::default()
        };
        let items = load_settings_items_for_config(
            "bmz-settings:profile",
            boot.profile_config.ui.locale(),
            &boot.app_config,
            &boot.profile_config,
            &extras,
        );
        let row = choice_row(&items, SettingsChoiceTarget::Profile);
        assert_eq!(row.choices[row.current].key, boot.profile_config.id);
        let other = choose(&row, "other");
        assert_eq!(other.label, "Other (other)");

        let effect = apply_settings_choice_to_config(
            &mut boot.app_config,
            &mut boot.profile_config,
            &data.paths,
            SettingsChoiceTarget::Profile,
            &other,
        );
        assert!(matches!(
            effect,
            SettingsChoiceEffect::SwitchProfile(ProfileManagerAction::Switch(id)) if id == "other"
        ));
        // The app's profile switch flow activates the profile, not the folder.
        assert_eq!(boot.profile_config.id, "default");
        assert_eq!(
            crate::config::load::load_app_config(&data.paths.config_toml).unwrap().active_profile,
            "default"
        );
        let current = SettingsChoice::new("default", "default");
        assert!(matches!(
            apply_settings_choice_to_config(
                &mut boot.app_config,
                &mut boot.profile_config,
                &data.paths,
                SettingsChoiceTarget::Profile,
                &current,
            ),
            SettingsChoiceEffect::Unchanged
        ));
    }

    #[test]
    fn library_choices_change_only_their_own_flag() {
        let data = ProfileTestDir::new();
        let mut boot = data.boot();
        boot.app_config.songs.roots = vec![
            PathEntry { path: "/songs/a".into(), enabled: true, recursive: true },
            PathEntry { path: "/songs/b".into(), enabled: true, recursive: true },
        ];
        boot.app_config.tables.sources = vec![
            DifficultyTableSource { url: "https://a.example/".into(), enabled: true },
            DifficultyTableSource { url: "https://b.example/".into(), enabled: false },
        ];
        let off = SettingsChoice::new("off", "OFF");
        let on = SettingsChoice::new("on", "ON");
        let mut apply = |target, choice: &SettingsChoice| {
            apply_settings_choice_to_config(
                &mut boot.app_config,
                &mut boot.profile_config,
                &data.paths,
                target,
                choice,
            )
        };
        assert!(matches!(
            apply(SettingsChoiceTarget::SongRootEnabled(1), &off),
            SettingsChoiceEffect::Library
        ));
        assert!(matches!(
            apply(SettingsChoiceTarget::SongRootEnabled(1), &off),
            SettingsChoiceEffect::Unchanged
        ));
        assert!(matches!(
            apply(SettingsChoiceTarget::SongRootRecursive(0), &off),
            SettingsChoiceEffect::Library
        ));
        assert!(matches!(
            apply(SettingsChoiceTarget::TableEnabled(1), &on),
            SettingsChoiceEffect::Library
        ));
        assert!(matches!(
            apply(SettingsChoiceTarget::TableEnabled(9), &on),
            SettingsChoiceEffect::Unchanged
        ));
        let roots = &boot.app_config.songs.roots;
        assert_eq!(
            roots.iter().map(|root| (root.enabled, root.recursive)).collect::<Vec<_>>(),
            [(true, false), (false, true)]
        );
        assert!(boot.app_config.tables.sources.iter().all(|source| source.enabled));
    }

    #[test]
    fn controller_assignment_matches_the_menu_buttons() {
        let data = ProfileTestDir::new();
        let mut boot = data.boot();
        boot.app_config.input.gamepad_slot_gilrs_ids = [Some(3), Some(4)];
        let pad = SettingsChoice::new("pad-a", "#0 Pad A");
        assert!(matches!(
            apply_settings_choice_to_config(
                &mut boot.app_config,
                &mut boot.profile_config,
                &data.paths,
                SettingsChoiceTarget::GamepadSlot(1),
                &pad,
            ),
            SettingsChoiceEffect::Input
        ));
        let input = &mut boot.app_config.input;
        assert_eq!(input.gamepad_slot_device_ids, [None, Some("pad-a".to_string())]);
        assert_eq!(input.gamepad_slot_gilrs_ids, [Some(3), None]);

        apply_gamepad_assignment_action(input, SettingsActionKind::GamepadSwap, &[]);
        assert_eq!(input.gamepad_slot_device_ids, [Some("pad-a".to_string()), None]);
        assert_eq!(input.gamepad_slot_gilrs_ids, [None, Some(3)]);
        let connected = ["pad-b".to_string(), "pad-c".to_string()];
        apply_gamepad_assignment_action(input, SettingsActionKind::GamepadAutoAssign, &connected);
        assert_eq!(
            input.gamepad_slot_device_ids,
            [Some("pad-b".to_string()), Some("pad-c".to_string())]
        );
        assert_eq!(input.gamepad_slot_gilrs_ids, [None, None]);
        apply_gamepad_assignment_action(input, SettingsActionKind::GamepadClearAssignment, &[]);
        assert_eq!(input.gamepad_slot_device_ids, [None, None]);
    }

    #[test]
    fn skin_choices_switch_skins_and_keep_each_skins_options() {
        let data = ProfileTestDir::new();
        let boot = data.boot();
        let skin_dir = data.paths.data_dir.join("skins").join("theme");
        std::fs::create_dir_all(&skin_dir).unwrap();
        let skin_path = skin_dir.join("select.json");
        std::fs::write(
            &skin_path,
            r#"{
                "type": 5, "name": "Theme Select",
                "property": [{ "name": "Theme", "item": [
                    { "name": "One", "op": 900 }, { "name": "Two", "op": 901 }
                ] }]
            }"#,
        )
        .unwrap();
        let skin_path = skin_path.to_string_lossy().into_owned();
        let catalog = crate::ui::SkinCatalog {
            select: vec![SkinCandidate {
                name: "Theme Select".into(),
                path: skin_path.clone(),
                origin: SkinCandidateOrigin::User,
            }],
            ..Default::default()
        };
        let target = Lr2SkinTarget::from_key("select").unwrap();
        let text = Localizer::new(boot.profile_config.ui.locale());
        let folder = |profile: &ProfileConfig| {
            let extras = SettingsFolderExtras {
                skin: Some(settings_skin_folder_data(
                    target,
                    &profile.skin,
                    &catalog,
                    &data.paths,
                    text,
                )),
                ..Default::default()
            };
            load_settings_items_for_config(
                "bmz-settings:skin:select",
                profile.ui.locale(),
                &boot.app_config,
                profile,
                &extras,
            )
        };
        let apply = |profile: &mut ProfileConfig, target, choice: &SettingsChoice| {
            apply_settings_choice_to_config(
                &mut boot.app_config.clone(),
                profile,
                &data.paths,
                target,
                choice,
            )
        };
        let mut profile = boot.profile_config.clone();
        profile.skin.select.clear();

        let items = folder(&profile);
        let skin_row = choice_row(&items, SettingsChoiceTarget::SkinPath(target));
        assert_eq!(skin_row.choices.len(), 2);
        assert_eq!(skin_row.value_text(), text.text("skin-default"));
        let theme = choose(&skin_row, &skin_path);
        assert!(matches!(
            apply(&mut profile, SettingsChoiceTarget::SkinPath(target), &theme),
            SettingsChoiceEffect::Skin(changed) if changed == target
        ));
        assert_eq!(profile.skin.select, skin_path);

        let items = folder(&profile);
        let option_row = choice_row(&items, SettingsChoiceTarget::SkinItem(target, 0));
        assert_eq!(option_row.label, "Theme");
        assert_eq!(option_row.value_text(), "One");
        let two = choose(&option_row, "Two");
        assert!(matches!(
            apply(&mut profile, SettingsChoiceTarget::SkinItem(target, 0), &two),
            SettingsChoiceEffect::Skin(_)
        ));
        assert_eq!(profile.skin.select_options.get("Theme").map(String::as_str), Some("Two"));

        // Like the BMZ menu, switching away and back restores the skin's options.
        let default = SettingsChoice::new("", "default");
        apply(&mut profile, SettingsChoiceTarget::SkinPath(target), &default);
        assert!(profile.skin.select_options.is_empty());
        apply(&mut profile, SettingsChoiceTarget::SkinPath(target), &theme);
        assert_eq!(profile.skin.select_options.get("Theme").map(String::as_str), Some("Two"));
        assert_eq!(
            choice_row(&folder(&profile), SettingsChoiceTarget::SkinItem(target, 0)).value_text(),
            "Two"
        );
    }
}
