use super::*;

fn gauge_document(extra: &str) -> SkinDocument {
    serde_json::from_str(&format!(
        r#"{{
            "w": 100, "h": 100,
            "image": [
                {{"id": "lit_red", "src": "lit"}}, {{"id": "lit_green", "src": "lit"}},
                {{"id": "dim_red", "src": "dim"}}, {{"id": "dim_green", "src": "dim"}}
            ],
            "gauge": {{"id": "g", "nodes": ["lit_red", "lit_green", "dim_red", "dim_green"],
                "parts": 10 {extra} }},
            "destination": [{{"id": "g", "dst": [{{"x": 0, "y": 0, "w": 100, "h": 10}}]}}]
        }}"#
    ))
    .unwrap()
}

fn sources() -> HashMap<String, SkinDocumentTexture> {
    ["lit", "dim"]
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                (*id).to_string(),
                SkinDocumentTexture {
                    source_id: (*id).to_string(),
                    texture: SkinTextureId(100 + index as u32),
                    source_size: SkinImageSize { width: 10.0, height: 10.0 },
                },
            )
        })
        .collect()
}

/// (lit parts, dim parts) drawn for a gauge of `final_value`.
fn lit_and_dim(
    document: &SkinDocument,
    result: bool,
    gauge_type: i32,
    final_value: f32,
    elapsed_ms: i32,
) -> (usize, usize) {
    let state = SkinDrawState {
        elapsed_ms,
        gauge: final_value,
        gauge_max: 100.0,
        gauge_border: 0.0,
        gauge_type,
        result_failed: result.then_some(false),
        ..SkinDrawState::default()
    };
    let items = document.static_render_items(&sources(), &state, &SkinTextState::default());
    let count = |id: u32| {
        items
            .iter()
            .filter(|item| matches!(item, SkinRenderItem::Image { texture, .. } if texture.0 == id))
            .count()
    };
    (count(100), count(101))
}

#[test]
fn stretch_value_follows_the_start_and_end_times() {
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, -5, 0, 500), 2.0);
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, 0, 0, 500), 2.0);
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, 250, 0, 500), 50.0);
    // Never beyond the final value, and exact at/after the end.
    assert_eq!(skin_gauge_result_stretch_value(30.0, 0.0, 100.0, 250, 0, 500), 30.0);
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, 500, 0, 500), 80.0);
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, 9999, 0, 500), 80.0);
    // Before starttime: the minimum; a start after the end shows the final value.
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, 100, 200, 600), 2.0);
    assert_eq!(skin_gauge_result_stretch_value(80.0, 2.0, 100.0, 0, 300, 300), 80.0);
    assert_eq!(skin_gauge_min_value(2), 2.0);
    assert_eq!(skin_gauge_min_value(3), 0.0);
}

#[test]
fn result_gauge_fills_up_over_starttime_and_endtime() {
    let document = gauge_document("");
    // HARD (3, minimum 0): 10 parts, final 100. Defaults are 0 .. 500 ms.
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 0), (0, 10));
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 250), (5, 5));
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 500), (10, 0));
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 5000), (10, 0));
    // The fill stops at the final value.
    assert_eq!(lit_and_dim(&document, true, 3, 30.0, 5000), (3, 7));
    // NORMAL never shows below 2, which still lights one part.
    assert_eq!(lit_and_dim(&document, true, 2, 100.0, 0), (1, 9));
}

#[test]
fn result_gauge_uses_the_skin_start_and_end_times() {
    let document = gauge_document(r#", "starttime": 1000, "endtime": 2000"#);
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 999), (0, 10));
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 1500), (5, 5));
    assert_eq!(lit_and_dim(&document, true, 3, 100.0, 2000), (10, 0));
}

#[test]
fn play_gauge_ignores_the_result_fill_animation() {
    let document = gauge_document("");
    assert_eq!(lit_and_dim(&document, false, 3, 100.0, 0), (10, 0));
    assert_eq!(lit_and_dim(&document, false, 3, 50.0, 0), (5, 5));
}
