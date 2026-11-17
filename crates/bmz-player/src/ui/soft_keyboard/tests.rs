use super::*;
use crate::i18n::AppLocale;

use SoftKeyboardAction as A;

fn open_text() -> SoftKeyboard {
    SoftKeyboard::open(SoftKeyboardFieldSpec::text("search"))
}

fn apply_all(keyboard: &mut SoftKeyboard, actions: &[SoftKeyboardAction]) {
    for &action in actions {
        keyboard.apply(action);
    }
}

#[test]
fn opens_at_top_left_of_first_mode_with_caret_at_end() {
    let keyboard =
        SoftKeyboard::open(SoftKeyboardFieldSpec::text("search").with_initial_text("abc"));
    assert_eq!(keyboard.mode(), SoftKeyboardMode::Hiragana);
    assert_eq!(keyboard.cursor(), (0, 0));
    assert_eq!(keyboard.selected_char(), Some('あ'));
    assert_eq!(keyboard.caret(), 3);
    assert_eq!(keyboard.text(), "abc");
}

#[test]
fn cursor_moves_and_wraps_in_both_axes() {
    let mut keyboard = open_text();
    assert_eq!(keyboard.apply(A::CursorRight), SoftKeyboardEvent::Moved);
    assert_eq!(keyboard.selected_char(), Some('か'));
    keyboard.apply(A::CursorLeft);
    keyboard.apply(A::CursorLeft);
    assert_eq!(keyboard.selected_char(), Some('ん'), "左端から右端へ折り返す");
    keyboard.apply(A::CursorRight);
    keyboard.apply(A::CursorDown);
    assert_eq!(keyboard.selected_char(), Some('い'));
    keyboard.apply(A::CursorUp);
    keyboard.apply(A::CursorUp);
    assert_eq!(keyboard.selected_char(), Some('お'), "上端から下端へ折り返す");
}

#[test]
fn cursor_skips_empty_cells() {
    let mut keyboard = open_text();
    keyboard.apply(A::MoveCursorTo { row: 0, column: 7 });
    assert_eq!(keyboard.selected_char(), Some('や'));
    keyboard.apply(A::CursorDown);
    assert_eq!(keyboard.selected_char(), Some('ゆ'));
    keyboard.apply(A::CursorDown);
    assert_eq!(keyboard.selected_char(), Some('よ'));
    keyboard.apply(A::CursorDown);
    assert_eq!(keyboard.selected_char(), Some('や'));

    keyboard.apply(A::MoveCursorTo { row: 1, column: 6 });
    assert_eq!(keyboard.selected_char(), Some('み'));
    keyboard.apply(A::CursorRight);
    assert_eq!(keyboard.selected_char(), Some('り'));

    assert_eq!(
        keyboard.apply(A::MoveCursorTo { row: 1, column: 7 }),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing),
        "空きマスは選べない"
    );
    assert_eq!(keyboard.selected_char(), Some('り'));
}

#[test]
fn every_row_and_column_with_cells_is_reachable() {
    for mode in SoftKeyboardMode::ALL {
        for (row_index, row) in mode.grid().iter().enumerate() {
            assert!(row.iter().any(Option::is_some), "{mode:?} row {row_index} is empty");
        }
    }
}

#[test]
fn input_inserts_selected_char_at_caret() {
    let mut keyboard = open_text();
    apply_all(&mut keyboard, &[A::InputSelected, A::CursorRight, A::InputSelected]);
    assert_eq!(keyboard.text(), "あか");
    apply_all(&mut keyboard, &[A::CaretLeft, A::CursorRight, A::InputSelected]);
    assert_eq!(keyboard.text(), "あさか");
    assert_eq!(keyboard.caret(), 2);
    keyboard.apply(A::Backspace);
    assert_eq!(keyboard.text(), "あか");
    assert_eq!(keyboard.apply(A::CaretRight), SoftKeyboardEvent::Moved);
    assert_eq!(
        keyboard.apply(A::CaretRight),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    );
    keyboard.apply(A::InputSpace);
    assert_eq!(keyboard.text(), "あか ");
    keyboard.apply(A::ClearAll);
    assert_eq!(keyboard.text(), "");
    assert_eq!(
        keyboard.apply(A::Backspace),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    );
}

#[test]
fn mode_switching_cycles_and_keeps_cursor_in_grid() {
    let mut keyboard = open_text();
    let mut seen = vec![keyboard.mode()];
    for _ in 0..SoftKeyboardMode::ALL.len() - 1 {
        keyboard.apply(A::NextMode);
        seen.push(keyboard.mode());
    }
    assert_eq!(seen, SoftKeyboardMode::ALL.to_vec());
    keyboard.apply(A::NextMode);
    assert_eq!(keyboard.mode(), SoftKeyboardMode::Hiragana, "末尾の次は先頭");
    keyboard.apply(A::PreviousMode);
    assert_eq!(keyboard.mode(), SoftKeyboardMode::WideSymbol);

    // 5 行のかなから 3 行の記号へ移ると、カーソルは文字盤の中へ収まる。
    keyboard.apply(A::SetMode(SoftKeyboardMode::Hiragana));
    keyboard.apply(A::MoveCursorTo { row: 4, column: 0 });
    keyboard.apply(A::SetMode(SoftKeyboardMode::Symbol));
    assert_eq!(keyboard.cursor(), (2, 0));
    assert_eq!(keyboard.selected_char(), Some('['));

    // 空きマスに当たる場合は、同じ行の近い文字へ寄せる。
    keyboard.apply(A::SetMode(SoftKeyboardMode::Hiragana));
    keyboard.apply(A::MoveCursorTo { row: 0, column: 10 });
    keyboard.apply(A::SetMode(SoftKeyboardMode::AlphaLower));
    assert_eq!(keyboard.selected_char(), Some('0'), "右端の空きマスから最も近い文字へ");

    // カタカナはひらがなと同じ位置。
    keyboard.apply(A::SetMode(SoftKeyboardMode::Hiragana));
    keyboard.apply(A::MoveCursorTo { row: 2, column: 3 });
    keyboard.apply(A::SetMode(SoftKeyboardMode::Katakana));
    assert_eq!(keyboard.selected_char(), Some('ツ'));
}

#[test]
fn convert_composes_dakuten_handakuten_and_small_kana() {
    let cases: &[(char, &[char])] = &[
        ('か', &['が', 'か']),
        ('は', &['ば', 'ぱ', 'は']),
        ('つ', &['っ', 'づ', 'つ']),
        ('う', &['ぅ', 'ゔ', 'う']),
        ('や', &['ゃ', 'や']),
        ('カ', &['ガ', 'ヵ', 'カ']),
        ('ホ', &['ボ', 'ポ', 'ホ']),
        ('a', &['A', 'a']),
    ];
    for &(start, expected) in cases {
        let mut ch = start;
        for &next in expected {
            ch = convert_char(ch).unwrap_or_else(|| panic!("{start} -> {ch} は変換できない"));
            assert_eq!(ch, next, "{start} の変換");
        }
    }
    assert_eq!(convert_char('ん'), None);
    assert_eq!(convert_char('1'), None);
}

#[test]
fn convert_applies_to_char_before_caret() {
    let mut keyboard = open_text();
    keyboard.apply(A::MoveCursorTo { row: 0, column: 5 });
    keyboard.apply(A::InputSelected);
    keyboard.apply(A::Convert);
    assert_eq!(keyboard.text(), "ば");
    keyboard.apply(A::Convert);
    assert_eq!(keyboard.text(), "ぱ");
    keyboard.apply(A::MoveCursorTo { row: 0, column: 10 });
    keyboard.apply(A::InputSelected);
    assert_eq!(
        keyboard.apply(A::Convert),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing),
        "ん は変換できない"
    );
    keyboard.apply(A::CaretLeft);
    keyboard.apply(A::Convert);
    assert_eq!(keyboard.text(), "はん");

    let mut empty = open_text();
    assert_eq!(
        empty.apply(A::Convert),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    );
}

#[test]
fn max_length_rejects_extra_input_but_allows_convert() {
    let mut keyboard = SoftKeyboard::open(SoftKeyboardFieldSpec::text("name").with_max_chars(2));
    keyboard.apply(A::MoveCursorTo { row: 0, column: 1 });
    apply_all(&mut keyboard, &[A::InputSelected, A::InputSelected]);
    assert_eq!(
        keyboard.apply(A::InputSelected),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::MaxLength)
    );
    assert_eq!(keyboard.notice(), Some(SoftKeyboardNotice::MaxLength));
    assert_eq!(
        keyboard.apply(A::InputSpace),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::MaxLength)
    );
    assert_eq!(keyboard.apply(A::Convert), SoftKeyboardEvent::TextChanged);
    assert_eq!(keyboard.text(), "かが");
    assert_eq!(keyboard.notice(), None, "受け付けた操作で通知は消える");

    let truncated = SoftKeyboard::open(
        SoftKeyboardFieldSpec::text("name").with_initial_text("あいうえお").with_max_chars(3),
    );
    assert_eq!(truncated.text(), "あいう");
    assert_eq!(truncated.caret(), 3);
}

#[test]
fn masked_field_hides_display_but_submits_real_text() {
    let mut keyboard = SoftKeyboard::open(
        SoftKeyboardFieldSpec::ascii("password").masked().with_initial_text("pw"),
    );
    keyboard.apply(A::InputSelected);
    assert_eq!(keyboard.display_text(), "●●●");
    assert_eq!(keyboard.text(), "pw1");
    keyboard.apply(A::Submit);
    assert_eq!(keyboard.take_outcome(), Some(SoftKeyboardOutcome::Submitted("pw1".into())));
}

#[test]
fn ascii_only_field_never_reaches_or_produces_kana() {
    let mut keyboard =
        SoftKeyboard::open(SoftKeyboardFieldSpec::ascii("url").with_initial_text("かa"));
    assert_eq!(keyboard.modes(), SoftKeyboardMode::ASCII);
    assert_eq!(keyboard.mode(), SoftKeyboardMode::AlphaLower);
    for _ in 0..10 {
        keyboard.apply(A::NextMode);
        assert!(keyboard.mode().is_ascii());
    }
    assert_eq!(
        keyboard.apply(A::SetMode(SoftKeyboardMode::Hiragana)),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed)
    );
    assert_eq!(keyboard.notice(), Some(SoftKeyboardNotice::NotAllowed));

    // 英字の大小は変換できる。
    keyboard.apply(A::Convert);
    assert_eq!(keyboard.text(), "かA");
    // 呼び出し側が渡した初期値のかなは、ASCII 以外へ変換しない。
    keyboard.apply(A::CaretLeft);
    assert_eq!(
        keyboard.apply(A::Convert),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    );
    assert_eq!(keyboard.text(), "かA");

    // 小文字だけの欄では大文字へ変換しない。
    let mut lower = SoftKeyboard::open(
        SoftKeyboardFieldSpec::ascii("id").with_modes(&[SoftKeyboardMode::AlphaLower]),
    );
    lower.apply(A::MoveCursorTo { row: 1, column: 0 });
    lower.apply(A::InputSelected);
    assert_eq!(
        lower.apply(A::Convert),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    );
    assert_eq!(
        lower.apply(A::NextMode),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    );
}

#[test]
fn space_can_be_disallowed() {
    let mut keyboard = SoftKeyboard::open(SoftKeyboardFieldSpec::ascii("code").without_space());
    assert_eq!(
        keyboard.apply(A::InputSpace),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed)
    );
    keyboard.apply(A::SetMode(SoftKeyboardMode::Symbol));
    keyboard.apply(A::MoveCursorTo { row: 2, column: 10 });
    assert_eq!(keyboard.selected_char(), Some(' '));
    assert_eq!(
        keyboard.apply(A::InputSelected),
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed)
    );
    assert_eq!(keyboard.text(), "");
}

#[test]
fn initial_mode_and_mode_restriction_are_normalized() {
    let keyboard = SoftKeyboard::open(
        SoftKeyboardFieldSpec::text("x")
            .with_modes(&[SoftKeyboardMode::Symbol, SoftKeyboardMode::Katakana])
            .with_initial_mode(SoftKeyboardMode::Symbol),
    );
    assert_eq!(keyboard.modes(), [SoftKeyboardMode::Katakana, SoftKeyboardMode::Symbol]);
    assert_eq!(keyboard.mode(), SoftKeyboardMode::Symbol);

    let fallback = SoftKeyboard::open(
        SoftKeyboardFieldSpec::ascii("x").with_initial_mode(SoftKeyboardMode::Hiragana),
    );
    assert_eq!(fallback.mode(), SoftKeyboardMode::AlphaLower);

    let empty = SoftKeyboard::open(SoftKeyboardFieldSpec::text("x").with_modes(&[]));
    assert_eq!(empty.modes(), SoftKeyboardMode::ALL);
}

#[test]
fn submit_finishes_once_and_ignores_later_actions() {
    let mut keyboard = open_text();
    keyboard.apply(A::InputSelected);
    assert_eq!(keyboard.apply(A::Submit), SoftKeyboardEvent::Submitted);
    assert!(keyboard.is_finished());
    assert_eq!(keyboard.apply(A::InputSelected), SoftKeyboardEvent::Ignored);
    assert_eq!(keyboard.apply(A::Cancel), SoftKeyboardEvent::Ignored);
    assert_eq!(keyboard.take_outcome(), Some(SoftKeyboardOutcome::Submitted("あ".into())));
    assert_eq!(keyboard.take_outcome(), None);
}

#[test]
fn cancel_is_immediate_when_unchanged_and_confirmed_when_edited() {
    let mut unchanged =
        SoftKeyboard::open(SoftKeyboardFieldSpec::text("x").with_initial_text("abc"));
    assert_eq!(unchanged.apply(A::Cancel), SoftKeyboardEvent::Cancelled);
    assert_eq!(unchanged.take_outcome(), Some(SoftKeyboardOutcome::Cancelled));

    let mut edited = open_text();
    edited.apply(A::InputSelected);
    assert_eq!(edited.apply(A::Cancel), SoftKeyboardEvent::CancelArmed);
    assert_eq!(edited.notice(), Some(SoftKeyboardNotice::CancelArmed));
    // ほかの操作をはさむと確認待ちは解除される。
    edited.apply(A::CursorRight);
    assert_eq!(edited.notice(), None);
    assert_eq!(edited.apply(A::Cancel), SoftKeyboardEvent::CancelArmed);
    assert_eq!(edited.apply(A::Cancel), SoftKeyboardEvent::Cancelled);
    assert_eq!(edited.take_outcome(), Some(SoftKeyboardOutcome::Cancelled));

    // 編集して元に戻した場合は、変更なしとして即座に閉じる。
    let mut restored = open_text();
    apply_all(&mut restored, &[A::InputSelected, A::Backspace]);
    assert_eq!(restored.apply(A::Cancel), SoftKeyboardEvent::Cancelled);
}

#[test]
fn pad_mapping_covers_keys_scratch_and_buttons() {
    use SoftKeyboardPadButton as B;
    let mut mapper = SoftKeyboardPadMapper::new();
    let expected = [
        (B::ScratchUp, A::CursorLeft),
        (B::ScratchDown, A::CursorRight),
        (B::Key(1), A::InputSelected),
        (B::Key(2), A::CursorUp),
        (B::Key(3), A::Convert),
        (B::Key(4), A::CursorDown),
        (B::Key(5), A::Backspace),
        (B::Key(6), A::NextMode),
        (B::Key(7), A::Submit),
        (B::E2, A::Cancel),
        (B::E3, A::PreviousMode),
        (B::E4, A::Cancel),
    ];
    for (button, action) in expected {
        assert_eq!(mapper.press(button), Some(action), "{button:?}");
        assert_eq!(mapper.release(button), None, "{button:?}");
    }
    assert_eq!(mapper.press(B::Key(0)), None);
    assert_eq!(mapper.press(B::Key(8)), None);
}

#[test]
fn pad_e1_tap_inputs_space_and_e1_hold_shifts_actions() {
    use SoftKeyboardPadButton as B;
    let mut mapper = SoftKeyboardPadMapper::new();
    assert_eq!(mapper.press(B::E1), None);
    assert!(mapper.e1_held());
    assert_eq!(mapper.release(B::E1), Some(A::InputSpace));
    assert!(!mapper.e1_held());

    assert_eq!(mapper.press(B::E1), None);
    assert_eq!(mapper.press(B::ScratchUp), Some(A::CaretLeft));
    assert_eq!(mapper.press(B::ScratchDown), Some(A::CaretRight));
    assert_eq!(mapper.press(B::Key(5)), Some(A::ClearAll));
    assert_eq!(mapper.press(B::Key(6)), Some(A::PreviousMode));
    assert_eq!(mapper.press(B::Key(1)), Some(A::InputSelected));
    assert_eq!(mapper.release(B::E1), None, "組み合わせた E1 は空白を入力しない");

    // reset 後の解放は無視する（フォーカス喪失時の再同期など）。
    mapper.press(B::E1);
    mapper.reset();
    assert_eq!(mapper.release(B::E1), None);
}

#[test]
fn pad_driven_session_types_japanese_search_text() {
    use SoftKeyboardPadButton as B;
    let mut keyboard = open_text();
    let mut mapper = SoftKeyboardPadMapper::new();
    let mut press = |keyboard: &mut SoftKeyboard, button| {
        if let Some(action) = mapper.press(button) {
            keyboard.apply(action);
        }
        if let Some(action) = mapper.release(button) {
            keyboard.apply(action);
        }
    };
    // 「ぱ」: 皿で は の列へ、入力して 2 回変換。
    for _ in 0..5 {
        press(&mut keyboard, B::ScratchDown);
    }
    press(&mut keyboard, B::Key(1));
    press(&mut keyboard, B::Key(3));
    press(&mut keyboard, B::Key(3));
    // 空白（E1 単独）、英字ページで "A"。
    press(&mut keyboard, B::E1);
    press(&mut keyboard, B::Key(6));
    press(&mut keyboard, B::Key(6));
    // 英字ページでも列は保つ（6 の列）。下へ 1 段で f、皿で左へ 5 列で a。
    press(&mut keyboard, B::Key(4));
    for _ in 0..5 {
        press(&mut keyboard, B::ScratchUp);
    }
    press(&mut keyboard, B::Key(1));
    press(&mut keyboard, B::Key(3));
    press(&mut keyboard, B::Key(7));
    assert_eq!(keyboard.take_outcome(), Some(SoftKeyboardOutcome::Submitted("ぱ A".into())));
}

#[test]
fn visible_labels_exist_in_desktop_catalog() {
    let keys = SoftKeyboardMode::ALL.map(SoftKeyboardMode::label_key).into_iter().chain([
        "soft-keyboard-space",
        "soft-keyboard-wide-space",
        "soft-keyboard-count",
        "soft-keyboard-count-max",
        "soft-keyboard-notice-max-length",
        "soft-keyboard-notice-not-allowed",
        "soft-keyboard-notice-cancel-armed",
        "soft-keyboard-help-scratch",
        "soft-keyboard-help-keys",
        "soft-keyboard-help-buttons",
        "soft-keyboard-help-e1-scratch",
        "soft-keyboard-help-e1-keys",
    ]);
    for key in keys {
        for locale in [AppLocale::Ja, AppLocale::En] {
            assert_ne!(crate::i18n::text(locale, key), key, "{locale:?} に {key} が無い");
        }
    }
}
