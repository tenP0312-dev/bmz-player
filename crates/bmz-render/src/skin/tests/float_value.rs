use super::*;

fn digits_source() -> HashMap<String, SkinDocumentTexture> {
    HashMap::from([(
        "digits".to_string(),
        SkinDocumentTexture {
            source_id: "digits".to_string(),
            texture: SkinTextureId(5),
            source_size: SkinImageSize { width: 120.0, height: 10.0 },
        },
    )])
}

fn float_document(extra: &str) -> SkinDocument {
    serde_json::from_str(&format!(
        r#"{{
            "w": 100, "h": 100,
            "source": [{{"id": "digits", "path": "digits.png"}}],
            "floatvalue": [{{"id": "f", "src": "digits", "w": 120, "h": 10,
                "divx": 12, "divy": 1, "iketa": 3, "fketa": 2, {extra} }}],
            "destination": [{{"id": "f", "dst": [{{"x": 10, "y": 20, "w": 10, "h": 10}}]}}]
        }}"#
    ))
    .unwrap()
}

/// (cell index, rect x in 100-wide canvas pixels) for each drawn image.
fn drawn(items: &[SkinRenderItem]) -> Vec<(i32, i32)> {
    items
        .iter()
        .filter_map(|item| match item {
            SkinRenderItem::Image { uv, rect, .. } => {
                Some(((uv.x * 12.0).round() as i32, (rect.x * 100.0).round() as i32))
            }
            _ => None,
        })
        .collect()
}

fn state_with_rate(ex_score: u32, total_notes: u32) -> SkinDrawState {
    SkinDrawState { ex_score, total_notes, ..SkinDrawState::default() }
}

#[test]
fn floatvalue_draws_a_rate_with_gain_in_play_and_result_paths() {
    // total_rate (1115) 750 / (500 * 2) = 0.75, gain 100 -> "75.00"
    let document = float_document(r#""ref": 1115, "gain": 100"#);
    let items = document.static_render_items(
        &digits_source(),
        &state_with_rate(750, 500),
        &SkinTextState::default(),
    );
    // 7 5 . 0 0; 12-cell sets put the point at cell 11.
    assert_eq!(
        drawn(&items),
        vec![(7, 10), (5, 20), (11, 30), (0, 40), (0, 50)],
        "left aligned, empty slot left on the right"
    );
}

#[test]
fn floatvalue_align_moves_digits_into_the_empty_slots() {
    let right = float_document(r#""ref": 1115, "gain": 100, "align": 1"#).static_render_items(
        &digits_source(),
        &state_with_rate(750, 500),
        &SkinTextState::default(),
    );
    // 6 slots, 5 drawn: one empty slot of 10 px.
    assert_eq!(drawn(&right).first(), Some(&(7, 20)));
    let center = float_document(r#""ref": 1115, "gain": 100, "align": 2, "space": 2"#)
        .static_render_items(
            &digits_source(),
            &state_with_rate(750, 500),
            &SkinTextState::default(),
        );
    assert_eq!(drawn(&center).first(), Some(&(7, 16)), "half of one empty slot (12 px)");
}

#[test]
fn floatvalue_zero_padding_fills_the_integer_digits() {
    let document = float_document(r#""ref": 1115, "gain": 100, "zeropadding": 1"#);
    let items = document.static_render_items(
        &digits_source(),
        &state_with_rate(750, 500),
        &SkinTextState::default(),
    );
    assert_eq!(
        drawn(&items).iter().map(|(cell, _)| *cell).collect::<Vec<_>>(),
        vec![0, 7, 5, 11, 0, 0]
    );
    let back = float_document(r#""ref": 1115, "gain": 100, "zeropadding": 2"#).static_render_items(
        &digits_source(),
        &state_with_rate(750, 500),
        &SkinTextState::default(),
    );
    // The back-zero cell is 10 in a 12-cell set.
    assert_eq!(drawn(&back).first().map(|(cell, _)| *cell), Some(10));
}

#[test]
fn floatvalue_hides_unknown_refs_and_missing_sources() {
    let unknown = float_document(r#""ref": 9999"#).static_render_items(
        &digits_source(),
        &state_with_rate(750, 500),
        &SkinTextState::default(),
    );
    assert!(drawn(&unknown).is_empty());
    let no_source = float_document(r#""ref": 1115"#).static_render_items(
        &HashMap::new(),
        &state_with_rate(750, 500),
        &SkinTextState::default(),
    );
    assert!(drawn(&no_source).is_empty());
}

#[test]
fn floatvalue_prefers_value_over_ref_and_accepts_names() {
    let document = float_document(r#""ref": 9999, "value": "total_rate", "gain": 100"#);
    let items = document.static_render_items(
        &digits_source(),
        &state_with_rate(500, 500),
        &SkinTextState::default(),
    );
    // 500 / 1000 = 0.5 -> "50.00"
    assert_eq!(
        drawn(&items).iter().map(|(cell, _)| *cell).collect::<Vec<_>>(),
        vec![5, 0, 11, 0, 0]
    );
}

#[test]
fn floatvalue_uses_the_minus_half_and_sign_cell_of_26_cell_images() {
    // 26 cells, 1 row. Plus: digits 0-9, 11 = point, 12 = sign. Minus: 13-22, 24 = point, 25 = sign.
    let document: SkinDocument = serde_json::from_str(
        r#"{
            "w": 100, "h": 100,
            "floatvalue": [{"id": "f", "src": "digits", "w": 260, "h": 10,
                "divx": 26, "divy": 1, "iketa": 2, "fketa": 1, "isSignvisible": true,
                "ref": 310}],
            "destination": [{"id": "f", "dst": [{"x": 0, "y": 0, "w": 10, "h": 10}]}]
        }"#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "digits".to_string(),
        SkinDocumentTexture {
            source_id: "digits".to_string(),
            texture: SkinTextureId(5),
            source_size: SkinImageSize { width: 260.0, height: 10.0 },
        },
    )]);
    let cells = |hispeed: f32| -> Vec<i32> {
        let state = SkinDrawState { hispeed, ..SkinDrawState::default() };
        document
            .static_render_items(&sources, &state, &SkinTextState::default())
            .iter()
            .filter_map(|item| match item {
                SkinRenderItem::Image { uv, .. } => Some((uv.x * 26.0).round() as i32),
                _ => None,
            })
            .collect()
    };
    assert_eq!(cells(3.5), vec![12, 3, 11, 5]);
    assert_eq!(cells(-3.5), vec![25, 16, 24, 18]);
}

#[test]
fn floatvalue_draws_in_the_select_screen_too() {
    let document = float_document(r#""ref": 368"#);
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            in_library: true,
            chart_total_gauge: 250.0,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        ..SelectSnapshot::default()
    };
    let items = document.select_render_items(&digits_source(), &snapshot);
    // "250.00" without padding: 2 5 0 . 0 0 (cells 2, 5, 0, point, 0, 0).
    assert_eq!(
        drawn(&items).iter().map(|(cell, _)| *cell).collect::<Vec<_>>(),
        vec![2, 5, 0, 11, 0, 0]
    );
}
