use super::*;

pub(in crate::skin) fn skin_builtin_value_f32(expr: &str, state: &SkinDrawState) -> Option<f32> {
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_rate:") {
        let slot = slot.parse::<i32>().ok()?;
        if !(1..=10).contains(&slot) {
            return None;
        }
        let (score, max_score) = ir_ranking_score_and_max(state, slot)?;
        return Some(if max_score == 0 { 0.0 } else { score as f32 / max_score as f32 });
    }
    if expr.trim() == "bmz:keylogger_nps" {
        return Some(state.keylogger_nps as f32);
    }
    if let Some(value) = keylogger_graph_value(expr.trim(), state) {
        return Some(value);
    }
    match expr.trim() {
        SKIN_EXPR_ADJUSTED_COVER => state.adjusted_cover_progress,
        SKIN_EXPR_ADJUSTED_RATE => state.adjusted_rate,
        SKIN_EXPR_ADJUSTED_RATE_ADOT => state.adjusted_rate_adot.map(|value| value as f32),
        SKIN_EXPR_FS_THRESHOLD => Some(state.fs_threshold_ms as f32),
        SKIN_EXPR_DEFAULT_CHART_TOTAL_COUNT => Some(default_chart_total_count_value(state)),
        SKIN_EXPR_DEFAULT_CHART_GAUGE => Some(default_chart_gauge_graph_value(state)),
        SKIN_EXPR_COURSE_CLEAR_RATE => Some(course_clear_rate_value(state)),
        SKIN_EXPR_GAUGE_PERCENT_INTEGER => {
            Some((clamped_gauge_value(state) * 100.0 / state.gauge_max.max(1.0)).floor())
        }
        SKIN_EXPR_GAUGE_PERCENT_FRACTION => {
            Some((clamped_gauge_value(state) * 10_000.0 / state.gauge_max.max(1.0)).floor() % 100.0)
        }
        SKIN_EXPR_GAUGE_AMOUNT_INTEGER => Some(clamped_gauge_value(state).floor()),
        SKIN_EXPR_GAUGE_AMOUNT_FRACTION => {
            Some((clamped_gauge_value(state) * 100.0).floor() % 100.0)
        }
        _ => None,
    }
}

pub(in crate::skin) fn keylogger_graph_value(expr: &str, state: &SkinDrawState) -> Option<f32> {
    let rest = expr.strip_prefix("bmz:keylogger_graph:")?;
    let mut parts = rest.split(':');
    let graph_kind = parts.next()?;
    let lane = parts.next()?.parse::<usize>().ok()?.checked_sub(1)?;
    let layer = parts.next()?;
    if parts.next().is_some() || lane >= LANE_COUNT {
        return None;
    }
    match graph_kind {
        "judge" => {
            let start = match layer {
                "cool" => 0,
                "great" => 1,
                "good" => 2,
                "bad" => 3,
                _ => return None,
            };
            let denominator_start = usize::from(state.keylogger_exclude_cool);
            let max = state
                .keylogger_judge_counts
                .iter()
                .map(|counts| counts[denominator_start..].iter().sum::<u32>())
                .max()
                .unwrap_or(0);
            let count = state.keylogger_judge_counts[lane][start..].iter().sum::<u32>();
            Some(if max == 0 { 0.0 } else { count as f32 / max as f32 })
        }
        "fastslow" => {
            let start = match layer {
                "cool" => 0,
                "fast" => 1,
                "slow" => 2,
                _ => return None,
            };
            let denominator_start = usize::from(state.keylogger_exclude_cool);
            let max = state
                .keylogger_fast_slow_counts
                .iter()
                .map(|counts| counts[denominator_start..].iter().sum::<u32>())
                .max()
                .unwrap_or(0);
            let count = state.keylogger_fast_slow_counts[lane][start..].iter().sum::<u32>();
            Some(if max == 0 { 0.0 } else { count as f32 / max as f32 })
        }
        _ => None,
    }
}

pub(in crate::skin) fn skin_builtin_value_i64(expr: &str, state: &SkinDrawState) -> Option<i64> {
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_rate_integer:") {
        let slot = slot.parse::<i32>().ok()?;
        return ir_ranking_score_rate_parts(state, slot).map(|parts| parts.0);
    }
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_rate_fraction:") {
        let slot = slot.parse::<i32>().ok()?;
        return ir_ranking_score_rate_parts(state, slot).map(|parts| parts.1);
    }
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_diff:") {
        let slot = slot.parse::<i32>().ok()?;
        if !(1..=10).contains(&slot) {
            return None;
        }
        let local_best = skin_state_number(170, state)?.max(skin_state_number(171, state)?);
        let ranking_score = ir_ranking_entry(&state.ir_ranking, slot - 1)?.ex_score?;
        return local_best.checked_sub(ranking_score);
    }
    if expr.trim() == "bmz:nearest_rank_diff_abs" {
        return nearest_grade_diff(state).map(|diff| diff.value.abs());
    }
    if expr.trim() == "bmz:wmii_next_rank_diff" {
        return wmii_next_rank_diff(state);
    }
    if expr.trim() == "bmz:wmii_next_rank_diff_no_max_minus" {
        return wmii_next_rank_diff_with_max_minus(state, false);
    }
    if expr.trim() == SKIN_EXPR_SELECT_TOTAL_NOTES_RATIO_INTEGER {
        return select_total_notes_ratio_parts(state).map(|parts| parts.0);
    }
    if expr.trim() == SKIN_EXPR_SELECT_TOTAL_NOTES_RATIO_FRACTION {
        return select_total_notes_ratio_parts(state).map(|parts| parts.1);
    }
    let number = skin_builtin_value_f32(expr, state)?;
    Some(match expr.trim() {
        SKIN_EXPR_DEFAULT_CHART_TOTAL_COUNT | SKIN_EXPR_DEFAULT_CHART_GAUGE => {
            number.round() as i64
        }
        _ => integer_property_value(number),
    })
}

pub(in crate::skin) fn select_total_notes_ratio_parts(state: &SkinDrawState) -> Option<(i64, i64)> {
    let total = skin_state_number(368, state)?.max(0);
    let notes = skin_state_number(74, state)?;
    if notes <= 0 {
        return Some((0, 0));
    }
    let scaled = i128::from(total).checked_mul(1_000)?.checked_div(i128::from(notes))?;
    Some((i64::try_from(scaled / 1_000).ok()?, i64::try_from(scaled % 1_000).ok()?))
}

pub(in crate::skin) fn skin_value_number(
    value: &SkinValueDef,
    state: &SkinDrawState,
) -> Option<i64> {
    if let Some(property) = &value.value {
        return skin_integer_property(property, state);
    }
    if value.id == "Number_Todayplayednotes" {
        return Some(player_stat_u64(daily_completed_notes(&state.player_stats.daily)));
    }
    if !value.expr.trim().is_empty() {
        return skin_state_number_expr(&value.expr, state);
    }
    if !value.value_expr.trim().is_empty() {
        if let Some(number) = evaluate_lua_number_expr(&value.value_expr, state) {
            return Some(number as i64);
        }
        if let Some(number) = skin_builtin_value_i64(&value.value_expr, state) {
            return Some(number);
        }
        return skin_state_float_expr(&value.value_expr, state).map(integer_property_value);
    }
    skin_state_number(value.ref_id, state)
}

pub(in crate::skin) fn skin_integer_property(
    property: &SkinPropertyValue,
    state: &SkinDrawState,
) -> Option<i64> {
    if let Some(id) = property.resolve_id(PropertyFamily::Integer) {
        return skin_state_number(id, state);
    }
    let expression = property.expression()?;
    evaluate_lua_number_expr(expression, state)
        .map(|number| number as i64)
        .or_else(|| skin_builtin_value_i64(expression, state))
        .or_else(|| skin_state_float_expr(expression, state).map(integer_property_value))
}

pub(in crate::skin) fn skin_rate_property(
    property: &SkinPropertyValue,
    state: &SkinDrawState,
) -> Option<f32> {
    if let Some(id) = property.resolve_id(PropertyFamily::Rate) {
        return skin_state_float_number(id, state);
    }
    let expression = property.expression()?;
    evaluate_lua_number_expr(expression, state)
        .map(|number| number as f32)
        .or_else(|| skin_builtin_value_f32(expression, state))
        .or_else(|| skin_state_float_expr(expression, state))
}

pub(in crate::skin) fn lua_value_callback_id(expr: &str) -> Option<usize> {
    expr.trim().strip_prefix(LUA_VALUE_CALLBACK_PREFIX).and_then(|id| id.parse::<usize>().ok())
}

pub(in crate::skin) fn evaluate_lua_number_expr(expr: &str, state: &SkinDrawState) -> Option<f64> {
    let callback_id = lua_value_callback_id(expr)?;
    let runtime = state.lua_runtime.as_ref()?;
    runtime.runtime.evaluate_number(
        callback_id,
        state,
        &runtime.enabled_options,
        &runtime.text_values,
    )
}

pub(in crate::skin) fn evaluate_lua_text_expr(expr: &str, state: &SkinDrawState) -> Option<String> {
    let callback_id = lua_value_callback_id(expr)?;
    let runtime = state.lua_runtime.as_ref()?;
    runtime.runtime.evaluate_text(
        callback_id,
        state,
        &runtime.enabled_options,
        &runtime.text_values,
    )
}

pub(in crate::skin) fn integer_property_value(value: f32) -> i64 {
    value as i64
}

pub(in crate::skin) fn skin_value_number_for_destination(
    value: &SkinValueDef,
    state: &SkinDrawState,
) -> Option<i64> {
    if let Some(level) = state.select_songlist_level_override {
        return Some(level);
    }
    if value.value.is_none()
        && value.ref_id == 0
        && value.expr.trim().is_empty()
        && value.value_expr.trim().is_empty()
    {
        return Some(if state.play_level != 0 {
            state.play_level
        } else {
            state.select_play_level
        });
    }
    skin_value_number(value, state)
}

/// SkinNumber.prepare evaluates its IntegerProperty before draw/timer checks.
/// The outer None leaves ordinary refs/expressions on their existing path;
/// Some(None) is a failed/hidden Lua number and must not be evaluated again.
pub(in crate::skin) fn prepare_lua_number_for_destination(
    value: Option<&SkinValueDef>,
    state: &SkinDrawState,
) -> Option<Option<i64>> {
    let value = value?;
    if !value.expr.trim().is_empty() || lua_value_callback_id(&value.value_expr).is_none() {
        return None;
    }
    Some(
        skin_value_number_for_destination(value, state)
            .filter(|number| *number != i64::from(i32::MIN) && *number != i64::from(i32::MAX)),
    )
}

pub(in crate::skin) fn skin_state_number_expr(expr: &str, state: &SkinDrawState) -> Option<i64> {
    let normalized = expr.replace('+', " + ").replace('-', " - ");
    let mut sign = 1_i64;
    let mut total = 0_i64;
    let mut expecting_value = true;
    for token in normalized.split_whitespace() {
        match token {
            "+" if expecting_value => sign = 1,
            "-" if expecting_value => sign = -1,
            "+" if !expecting_value => {
                sign = 1;
                expecting_value = true;
            }
            "-" if !expecting_value => {
                sign = -1;
                expecting_value = true;
            }
            value => {
                if !expecting_value {
                    return None;
                }
                let term = skin_state_number_expr_term(value, state)?;
                total += sign * term;
                sign = 1;
                expecting_value = false;
            }
        }
    }
    if expecting_value {
        return None;
    }
    Some(total)
}

/// Value of a `floatvalue` before `gain`. `None` hides the number.
pub(in crate::skin) fn skin_float_value(
    def: &SkinFloatValueDef,
    state: &SkinDrawState,
) -> Option<f32> {
    // `value` wins over `ref`.
    if let Some(property) = &def.value {
        return match property {
            SkinPropertyValue::Id(id) => skin_state_rate_value(*id, state),
            SkinPropertyValue::Expression(expression) => {
                if let Some(id) = property.resolve_id(PropertyFamily::Float) {
                    return skin_state_float_value(id, state);
                }
                if let Some(id) = property.resolve_id(PropertyFamily::Rate) {
                    return skin_state_rate_value(id, state);
                }
                skin_float_expression_value(expression, state)
            }
        };
    }
    if !def.value_expr.trim().is_empty() {
        return skin_float_expression_value(&def.value_expr, state);
    }
    skin_state_float_value(def.ref_id, state)
}

fn skin_float_expression_value(expression: &str, state: &SkinDrawState) -> Option<f32> {
    if lua_value_callback_id(expression).is_some() {
        return evaluate_lua_number_expr(expression, state).map(|number| number as f32);
    }
    skin_builtin_value_f32(expression, state).or_else(|| skin_state_float_expr(expression, state))
}

/// `floatvalue` with a numeric `ref`: the float table first, then the rate table.
pub(in crate::skin) fn skin_state_float_value(ref_id: i32, state: &SkinDrawState) -> Option<f32> {
    skin_state_float_table_value(ref_id, state).or_else(|| skin_state_rate_value(ref_id, state))
}

fn skin_state_rate_value(ref_id: i32, state: &SkinDrawState) -> Option<f32> {
    skin_state_float_number(ref_id, state)
}

/// Float-only ids (the rate table is consulted separately).
fn skin_state_float_table_value(ref_id: i32, state: &SkinDrawState) -> Option<f32> {
    let notes = judge_rate_total_notes(state);
    let ratio = |numerator: u32, denominator: u32| -> f32 {
        if denominator > 0 { numerator as f32 / denominator as f32 } else { 0.0 }
    };
    match ref_id {
        // score_rate: the score rate at the current point of play.
        1102 => Some(current_score_rate_value(state)),
        // total_rate / score_rate2: the score over the whole chart.
        1115 | 155 => Some(ratio(state.ex_score, state.total_notes.saturating_mul(2))),
        85 => Some(ratio(state.judge_counts.pgreat, notes)),
        86 => Some(ratio(state.judge_counts.great, notes)),
        87 => Some(ratio(state.judge_counts.good, notes)),
        88 => Some(ratio(state.judge_counts.bad, notes)),
        89 => Some(ratio(state.judge_counts.poor, notes)),
        183 => {
            let max = state.total_notes.saturating_mul(2);
            Some(ratio(result_mybest_ex_score(state).unwrap_or(0), max))
        }
        122 | 135 | 157 => {
            let max = state.total_notes.saturating_mul(2);
            Some(ratio(state.target_ex_score.unwrap_or(0), max))
        }
        // Result: the integer part of the average display time / timing error / deviation in
        // ms, truncated toward zero. The two decimals are 373 / 375 / 377 (spec chapter 11).
        372 => state.average_duration_us.map(|us| (us / 1_000) as f32),
        374 => state.average_timing_ms.map(f32::trunc),
        376 => state.stddev_timing_ms.map(f32::trunc),
        310 => Some(state.hispeed_display.unwrap_or(state.hispeed)),
        1107 => Some(clamped_gauge_value(state)),
        367 => Some(state.select_chart_density),
        362 => Some(state.select_chart_end_density),
        360 => Some(state.select_chart_peak_density),
        368 => Some(state.select_chart_total_gauge),
        165 => Some(state.resource_load_progress.clamp(0.0, 1.0)),
        203..=225 if ref_id % 2 == 1 => ir_clear_rate_value(&state.ir_ranking, ref_id),
        227 | 229 => ir_clear_rate_value(&state.ir_ranking, ref_id),
        _ => None,
    }
}

fn judge_rate_total_notes(state: &SkinDrawState) -> u32 {
    if state.select_screen {
        state.select_total_notes.max(state.total_notes)
    } else {
        state.past_notes
    }
}

/// IR clear-lamp shares (0..1). The odd ids are the fraction half of the integer ids.
fn ir_clear_rate_value(ranking: &crate::scene::ResultIrSnapshot, ref_id: i32) -> Option<f32> {
    if ranking.state != crate::scene::ResultIrState::Loaded {
        return None;
    }
    let Some(counts) = ranking.clear_counts else {
        // Providers without the population only give a rounded total clear rate.
        return (ref_id == 227).then(|| ranking.clear_rate.map(|rate| rate as f32 / 100.0))?;
    };
    let total: u64 = counts.iter().map(|&count| u64::from(count)).sum();
    if total == 0 {
        return Some(0.0);
    }
    let count: u64 = match ref_id {
        227 => counts[2..].iter().map(|&c| u64::from(c)).sum(),
        229 => counts[8..].iter().map(|&c| u64::from(c)).sum(),
        _ => {
            let index = match ref_id - 1 {
                202 => 0,
                210 => 1,
                204 => 2,
                206 => 3,
                212 => 4,
                214 => 5,
                216 => 6,
                208 => 7,
                218 => 8,
                222 => 9,
                224 => 10,
                _ => return None,
            };
            u64::from(counts[index])
        }
    };
    Some(count as f32 / total as f32)
}
