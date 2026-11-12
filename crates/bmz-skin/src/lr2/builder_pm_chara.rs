//! LR2 CSV の `PM_CHARA` 系命令を、JSON の `pmchara` 部品へ変換する。
//!
//! 仕様: docs/bmsir/SKIN_BEHAVIOR_SPEC.md 3 章。`.chp` の読み込みとアニメーションは
//! JSON の `pmchara` と同じ経路（`bmz_skin::load_pm_chara_with_context`）を使う。

use super::*;

impl<'a> CsvBuilder<'a> {
    /// `#DST_PM_CHARA_1P` / `_2P`: x, y, w, h, color, offset, folderpath。
    pub(super) fn add_pm_chara_play(&mut self, line: &CsvLine, side: i32) {
        let values = parse_values(line);
        let destination =
            [values[1], values[2], values[3], values[4], values[5], 0, 0, 0, 0, values[6]];
        self.add_pm_chara_part(0, side, &destination, field(line, 7));
    }

    /// `#DST_PM_CHARA_ANIMATION`: x, y, w, h, color, animationtype, timer, op1-3, offset, folderpath。
    pub(super) fn add_pm_chara_animation(&mut self, line: &CsvLine) {
        let values = parse_values(line);
        let animation_type = values[6];
        if !(0..=9).contains(&animation_type) {
            return;
        }
        let destination = [
            values[1], values[2], values[3], values[4], values[5], values[7], values[8], values[9],
            values[10], values[11],
        ];
        // BMZ の pmchara type 6..15 は、仕様の animationtype 0..9 と同じ並び。
        self.add_pm_chara_part(animation_type + 6, 1, &destination, field(line, 12));
    }

    /// `#SRC_PM_CHARA_IMAGE`: color, type, folderpath。置き場所は続く `#DST_PM_CHARA_IMAGE`。
    pub(super) fn add_pm_chara_image(&mut self, line: &CsvLine) {
        let image_type = parse_i32(line.fields.get(2));
        if !(0..=4).contains(&image_type) {
            self.current = None;
            return;
        }
        let color = pm_chara_color(parse_i32(line.fields.get(1)));
        let id = self.register_pm_chara(image_type + 1, 1, color, field(line, 3));
        self.set_current(id);
    }

    fn add_pm_chara_part(
        &mut self,
        chara_type: i32,
        side: i32,
        // x, y, w, h, color, timer, op1, op2, op3, offset
        destination: &[i32; 10],
        folder: &str,
    ) {
        let color = pm_chara_color(destination[4]);
        let id = self.register_pm_chara(chara_type, side, color, folder);
        self.set_current(id);
        let mut fields = vec!["#DST_IMAGE".to_string()];
        // index, time, x, y, w, h, acc, a, r, g, b, blend, filter, angle, center, loop,
        // timer, op1, op2, op3, offset
        let columns = [
            0,
            0,
            destination[0],
            destination[1],
            destination[2],
            destination[3],
            0,
            255,
            255,
            255,
            255,
            1,
            0,
            0,
            0,
            0,
            destination[5],
            destination[6],
            destination[7],
            destination[8],
            destination[9],
        ];
        fields.extend(columns.iter().map(i32::to_string));
        let destination_line = CsvLine { command: "DST_IMAGE".to_string(), fields };
        self.add_destination(&destination_line);
    }

    fn register_pm_chara(
        &mut self,
        chara_type: i32,
        side: i32,
        color: i32,
        folder: &str,
    ) -> String {
        let index = self.pm_charas.len();
        let id = format!("lr2-pmchara:{index}");
        let source_id = format!("lr2-pmchara-src:{index}");
        let path = self.resolve_source_path(folder);
        self.sources.push(json!({ "id": source_id, "path": path }));
        self.pm_charas.push(json!({
            "id": id,
            "src": source_id,
            "color": color,
            "type": chara_type,
            "side": side,
        }));
        id
    }
}

/// 1 以外（2 を除く）は 1P の色。
fn pm_chara_color(color: i32) -> i32 {
    if color == 2 { 2 } else { 1 }
}
