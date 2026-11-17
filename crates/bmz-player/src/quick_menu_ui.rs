//! E4 短押しで開くクイックメニューの egui 表示。
//!
//! 操作はアプリ側がコントローラー入力で行い、ここでは表示だけを作る。マウス・キーボードでは操作しない。

use egui::{Context, RichText};

/// 1フレーム分の表示内容。文言は呼び出し側で翻訳済み。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickMenuPanel {
    pub title: String,
    pub items: Vec<QuickMenuPanelItem>,
    pub cursor: usize,
    /// 操作の案内（皿で選ぶ、奇数鍵で実行など）。
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickMenuPanelItem {
    pub label: String,
    /// 近道の表示（例: `E4+KEY1`）。
    pub shortcut: Option<String>,
}

pub fn build_quick_menu_panel(ctx: &Context, panel: &QuickMenuPanel) {
    egui::Area::new(egui::Id::new("bmz_quick_menu"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::window(ui.style())
                .fill(egui::Color32::from_rgba_unmultiplied(16, 20, 32, 236))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    ui.label(RichText::new(&panel.title).strong().size(18.0));
                    ui.separator();
                    for (index, item) in panel.items.iter().enumerate() {
                        let selected = index == panel.cursor;
                        ui.horizontal(|ui| {
                            let marker = if selected { "▶" } else { "  " };
                            let mut label = RichText::new(format!("{marker} {}", item.label));
                            if selected {
                                label = label.strong().color(egui::Color32::from_rgb(255, 214, 92));
                            }
                            ui.label(label);
                            if let Some(shortcut) = item.shortcut.as_deref() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(RichText::new(shortcut).weak().monospace());
                                    },
                                );
                            }
                        });
                    }
                    ui.separator();
                    ui.label(RichText::new(&panel.hint).weak().small());
                });
        });
}
