//! 画面キーボードの egui 描画。状態は変更せず、マウス操作を [`SoftKeyboardAction`] で返す。

use egui::{Color32, Context, RichText, Stroke};

use super::layout::SoftKeyboardMode;
use super::state::{SoftKeyboard, SoftKeyboardAction, SoftKeyboardNotice};
use crate::i18n::{FluentArgs, Localizer};

const CELL_SIZE: f32 = 34.0;
const SELECTED_FILL: Color32 = Color32::from_rgb(64, 120, 220);
const PANEL_FILL: Color32 = Color32::from_rgba_premultiplied(16, 20, 32, 235);

/// 描画中に受け付けたマウス操作。呼び出し側が [`SoftKeyboard::apply`] へ順に渡す。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SoftKeyboardDrawOutput {
    pub actions: Vec<SoftKeyboardAction>,
}

/// 画面キーボードを画面中央の前面ウィンドウとして描く。
///
/// `e1_held` が true なら、E1 hold 中の操作を案内に出す（[`super::SoftKeyboardPadMapper::e1_held`]）。
pub fn draw_soft_keyboard(
    ctx: &Context,
    keyboard: &SoftKeyboard,
    e1_held: bool,
    text: Localizer,
) -> SoftKeyboardDrawOutput {
    let mut output = SoftKeyboardDrawOutput::default();
    egui::Window::new(keyboard.spec().label.as_str())
        .id(egui::Id::new("bmz_soft_keyboard"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .movable(false)
        .resizable(false)
        .collapsible(false)
        .frame(egui::Frame::window(ctx.global_style().as_ref()).fill(PANEL_FILL))
        .show(ctx, |ui| {
            draw_text_line(ui, keyboard, text);
            ui.add_space(4.0);
            draw_mode_tabs(ui, keyboard, text, &mut output);
            ui.add_space(4.0);
            draw_grid(ui, keyboard, text, &mut output);
            ui.add_space(4.0);
            draw_notice(ui, keyboard, text);
            ui.separator();
            draw_help(ui, e1_held, text);
        });
    output
}

fn draw_text_line(ui: &mut egui::Ui, keyboard: &SoftKeyboard, text: Localizer) {
    let shown: Vec<char> = keyboard.display_text().chars().collect();
    let caret = keyboard.caret().min(shown.len());
    let before: String = shown[..caret].iter().collect();
    let after: String = shown[caret..].iter().collect();
    egui::Frame::new()
        .fill(Color32::from_black_alpha(160))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_width(CELL_SIZE * super::layout::GRID_COLUMNS as f32);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(RichText::new(before).monospace().size(18.0));
                ui.label(RichText::new("|").monospace().size(18.0).color(SELECTED_FILL));
                ui.label(RichText::new(after).monospace().size(18.0));
            });
        });
    let mut args = FluentArgs::new();
    args.set("count", keyboard.char_count() as i64);
    let count = match keyboard.spec().max_chars {
        Some(max) => {
            args.set("max", max as i64);
            text.format("soft-keyboard-count-max", &args)
        }
        None => text.format("soft-keyboard-count", &args),
    };
    ui.label(RichText::new(count).weak().small());
}

fn draw_mode_tabs(
    ui: &mut egui::Ui,
    keyboard: &SoftKeyboard,
    text: Localizer,
    output: &mut SoftKeyboardDrawOutput,
) {
    ui.horizontal(|ui| {
        for &mode in keyboard.modes() {
            let selected = mode == keyboard.mode();
            if ui.selectable_label(selected, text.text(mode.label_key())).clicked() && !selected {
                output.actions.push(SoftKeyboardAction::SetMode(mode));
            }
        }
    });
}

fn draw_grid(
    ui: &mut egui::Ui,
    keyboard: &SoftKeyboard,
    text: Localizer,
    output: &mut SoftKeyboardDrawOutput,
) {
    let (cursor_row, cursor_column) = keyboard.cursor();
    let mode = keyboard.mode();
    egui::Grid::new(("bmz_soft_keyboard_grid", mode)).spacing([2.0, 2.0]).show(ui, |ui| {
        for (row_index, row) in mode.grid().iter().enumerate() {
            for (column_index, cell) in row.iter().enumerate() {
                let Some(ch) = *cell else {
                    ui.allocate_exact_size(egui::vec2(CELL_SIZE, CELL_SIZE), egui::Sense::hover());
                    continue;
                };
                let selected = row_index == cursor_row && column_index == cursor_column;
                let label = cell_label(ch, mode, text);
                let mut button = egui::Button::new(RichText::new(label).size(17.0))
                    .min_size(egui::vec2(CELL_SIZE, CELL_SIZE));
                if selected {
                    button =
                        button.fill(SELECTED_FILL).stroke(Stroke::new(2.0_f32, Color32::WHITE));
                }
                if ui.add(button).clicked() {
                    output.actions.push(SoftKeyboardAction::MoveCursorTo {
                        row: row_index,
                        column: column_index,
                    });
                    output.actions.push(SoftKeyboardAction::InputSelected);
                }
            }
            ui.end_row();
        }
    });
}

fn cell_label(ch: char, mode: SoftKeyboardMode, text: Localizer) -> String {
    match ch {
        ' ' => text.text("soft-keyboard-space"),
        '\u{3000}' if mode == SoftKeyboardMode::WideSymbol => text.text("soft-keyboard-wide-space"),
        _ => ch.to_string(),
    }
}

fn draw_notice(ui: &mut egui::Ui, keyboard: &SoftKeyboard, text: Localizer) {
    let key = match keyboard.notice() {
        Some(SoftKeyboardNotice::MaxLength) => "soft-keyboard-notice-max-length",
        Some(SoftKeyboardNotice::NotAllowed) => "soft-keyboard-notice-not-allowed",
        Some(SoftKeyboardNotice::CancelArmed) => "soft-keyboard-notice-cancel-armed",
        None => {
            // 通知の有無で高さが変わらないよう、空行を確保する。
            ui.label(" ");
            return;
        }
    };
    ui.label(RichText::new(text.text(key)).color(Color32::from_rgb(255, 200, 80)));
}

fn draw_help(ui: &mut egui::Ui, e1_held: bool, text: Localizer) {
    let keys: &[&str] = if e1_held {
        &["soft-keyboard-help-e1-scratch", "soft-keyboard-help-e1-keys"]
    } else {
        &["soft-keyboard-help-scratch", "soft-keyboard-help-keys", "soft-keyboard-help-buttons"]
    };
    for key in keys {
        ui.label(RichText::new(text.text(key)).small());
    }
}
