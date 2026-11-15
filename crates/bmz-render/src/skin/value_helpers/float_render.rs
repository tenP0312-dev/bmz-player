use super::*;

/// Draws a `floatvalue` as digit images. Returns no items when the value is hidden.
pub(in crate::skin) fn float_value_render_items(
    document: &SkinDocument,
    def: &SkinFloatValueDef,
    frame: ResolvedSkinFrame,
    elapsed_ms: i32,
    state: &SkinDrawState,
    sources: &HashMap<String, SkinDocumentTexture>,
) -> Vec<SkinRenderItem> {
    let Some(source) = sources.get(&def.src) else {
        return Vec::new();
    };
    let divx = def.divx.max(1);
    let divy = def.divy.max(1);
    let Some(cells) = FloatCellSet::from_cell_count(divx.saturating_mul(divy)) else {
        return Vec::new();
    };
    let Some(raw) = skin_float_value(def, state) else {
        return Vec::new();
    };
    let value = raw * def.gain;
    let Some(layout) = layout_float_number(
        value,
        def.iketa,
        def.fketa,
        def.sign_visible,
        def.zeropadding,
        cells.has_sign_cell(),
    ) else {
        return Vec::new();
    };
    let width_px = if def.w == -1 { source.source_size.width.round() as i32 } else { def.w };
    let height_px = if def.h == -1 { source.source_size.height.round() as i32 } else { def.h };
    let cell_width = (width_px / divx) as f32;
    let cell_height = (height_px / divy) as f32;
    if cell_width <= 0.0 || cell_height <= 0.0 {
        return Vec::new();
    }
    let source_width = source.source_size.width.max(1.0);
    let source_height = source.source_size.height.max(1.0);
    // Groups after the first are animation frames.
    let group = if def.cycle > 0 && cells.groups > 1 {
        (elapsed_ms.rem_euclid(def.cycle) * cells.groups / def.cycle).min(cells.groups - 1)
    } else {
        0
    };
    let step = frame.w + def.space;
    // Empty slots sit at the right end; align moves the digits into them.
    let shift = match def.align {
        1 => step * layout.empty_slots as i32,
        2 => step * layout.empty_slots as i32 / 2,
        _ => 0,
    };
    let negative = layout.negative && cells.has_minus_half();
    layout
        .glyphs
        .iter()
        .enumerate()
        .map(|(index, glyph)| {
            let cell = group * cells.stride + cells.cell(*glyph, negative);
            let uv = TextureRegion {
                x: (def.x as f32 + cell_width * (cell % divx) as f32) / source_width,
                y: (def.y as f32 + cell_height * (cell / divx) as f32) / source_height,
                width: cell_width / source_width,
                height: cell_height / source_height,
            };
            let rect = normalize_skin_frame_rect(
                ResolvedSkinFrame {
                    x: frame.x + step * index as i32 + shift,
                    y: frame.y,
                    w: frame.w,
                    h: frame.h,
                    ..frame
                },
                document.w,
                document.h,
            );
            skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                0,
                BlendMode::Normal,
                Some(source.source_size),
                false,
            )
        })
        .collect()
}
