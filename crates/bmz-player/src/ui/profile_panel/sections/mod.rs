use super::*;

mod arena;
mod core;
mod display;
mod ir;
mod key_config;
mod play;
mod select;
mod ui;

pub(super) use arena::*;
pub(super) use core::*;
pub(super) use display::*;
pub(super) use ir::*;
pub(super) use key_config::*;
pub(super) use play::*;
pub(super) use select::*;
pub(super) use ui::*;

pub(super) struct ProfileSectionContext<'a> {
    pub(super) profile: &'a mut ProfileConfig,
    pub(super) app_config: &'a mut AppConfig,
    pub(super) show_fps: &'a mut bool,
    pub(super) ir_login: &'a mut IrLoginUiState,
    pub(super) ir_device_key: &'a mut IrDeviceKeyUiState,
    pub(super) ir_manual: &'a mut IrManualUi,
    pub(super) profile_manager: &'a mut ProfileManagerUiState,
    pub(super) key_config: &'a mut EguiKeyConfigUiState,
    pub(super) profile_root: &'a std::path::Path,
    pub(super) arena: &'a crate::arena::connection::ArenaHandle,
    pub(super) unrestricted: bool,
    pub(super) text: Localizer,
    pub(super) save_clicked: bool,
    pub(super) save_app_config: bool,
    pub(super) key_config_action: Option<EguiKeyConfigAction>,
}
