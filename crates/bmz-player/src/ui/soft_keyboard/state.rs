//! 画面キーボードの状態機械。egui に依存せず、操作（[`SoftKeyboardAction`]）だけで進む。

use super::layout::{GRID_COLUMNS, SoftKeyboardMode, char_in_modes, convert_char};

/// パスワード欄で文字の代わりに表示する記号。
pub const MASK_CHAR: char = '●';

/// 画面キーボードで入力する欄の仕様。開くときに呼び出し側が渡す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftKeyboardFieldSpec {
    /// 欄の名前（表示用。呼び出し側で翻訳済みの文字列を渡す）。
    pub label: String,
    /// 開いたときの文字列。`max_chars` を超える分は切り捨てる。
    pub initial_text: String,
    /// 入力できる最大の文字数（Unicode scalar 単位）。`None` は無制限。
    pub max_chars: Option<usize>,
    /// true なら入力した文字を [`MASK_CHAR`] で隠して表示する。
    pub masked: bool,
    /// 使える文字種。切替順は [`SoftKeyboardMode::ALL`] の順にそろえる。空なら全種。
    pub modes: Vec<SoftKeyboardMode>,
    /// 開いたときの文字種。`modes` に無い場合は `modes` の先頭。
    pub initial_mode: Option<SoftKeyboardMode>,
    /// 半角空白を入力できるか。
    pub allow_space: bool,
}

impl SoftKeyboardFieldSpec {
    /// すべての文字種（かな・英数字・記号）を使える欄。曲名の検索など。
    pub fn text(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            initial_text: String::new(),
            max_chars: None,
            masked: false,
            modes: SoftKeyboardMode::ALL.to_vec(),
            initial_mode: None,
            allow_space: true,
        }
    }

    /// ASCII の文字種だけを使える欄。URL・パス・メール・パスワードなど。
    pub fn ascii(label: impl Into<String>) -> Self {
        Self { modes: SoftKeyboardMode::ASCII.to_vec(), ..Self::text(label) }
    }

    pub fn with_initial_text(mut self, text: impl Into<String>) -> Self {
        self.initial_text = text.into();
        self
    }

    pub fn with_max_chars(mut self, max_chars: usize) -> Self {
        self.max_chars = Some(max_chars);
        self
    }

    pub fn masked(mut self) -> Self {
        self.masked = true;
        self
    }

    pub fn with_modes(mut self, modes: &[SoftKeyboardMode]) -> Self {
        self.modes = modes.to_vec();
        self
    }

    pub fn with_initial_mode(mut self, mode: SoftKeyboardMode) -> Self {
        self.initial_mode = Some(mode);
        self
    }

    pub fn without_space(mut self) -> Self {
        self.allow_space = false;
        self
    }
}

/// 画面キーボードへの操作。物理ボタンとの対応は [`super::pad`] にある。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftKeyboardAction {
    /// 文字盤のカーソルを上へ（上端では下端へ）。
    CursorUp,
    /// 文字盤のカーソルを下へ（下端では上端へ）。
    CursorDown,
    /// 文字盤のカーソルを左へ（左端では右端へ）。
    CursorLeft,
    /// 文字盤のカーソルを右へ（右端では左端へ）。
    CursorRight,
    /// 文字盤のマスを直接選ぶ（マウス操作用）。
    MoveCursorTo { row: usize, column: usize },
    /// カーソル位置の文字を入力する。
    InputSelected,
    /// 半角空白を入力する。
    InputSpace,
    /// キャレットの直前の文字を変換する（濁点・半濁点・小書き、英字の大小）。
    Convert,
    /// キャレットの直前の 1 文字を消す。
    Backspace,
    /// 入力中の文字列をすべて消す。
    ClearAll,
    /// 入力中の文字列の中でキャレットを左へ。
    CaretLeft,
    /// 入力中の文字列の中でキャレットを右へ。
    CaretRight,
    /// 次の文字種へ（末尾では先頭へ）。
    NextMode,
    /// 前の文字種へ（先頭では末尾へ）。
    PreviousMode,
    /// 文字種を直接選ぶ（マウス操作用）。使えない文字種は無視する。
    SetMode(SoftKeyboardMode),
    /// 入力を確定する。
    Submit,
    /// 入力を取り消す。文字列を変更済みなら 1 回目は確認待ちにする。
    Cancel,
}

/// 操作が受け付けられなかった理由。描画側は短い通知に使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftKeyboardRejection {
    /// 最大の文字数に達している。
    MaxLength,
    /// この欄では使えない文字。
    NotAllowed,
    /// 空きマス・空の文字列など、対象が無い。
    Nothing,
}

/// 操作 1 回の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftKeyboardEvent {
    /// 確定・取消の後など、何もしなかった。
    Ignored,
    /// カーソル・キャレットが動いた、または文字種が変わった。
    Moved,
    /// 文字列が変わった。
    TextChanged,
    /// 受け付けなかった。
    Rejected(SoftKeyboardRejection),
    /// 取消の確認待ちになった。もう一度 Cancel で取り消す。
    CancelArmed,
    /// 確定した。結果は [`SoftKeyboard::take_outcome`] で受け取る。
    Submitted,
    /// 取り消した。
    Cancelled,
}

/// 画面キーボードを閉じたときの結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoftKeyboardOutcome {
    Submitted(String),
    Cancelled,
}

/// 描画側に出す通知。次に受け付けた操作で消える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftKeyboardNotice {
    MaxLength,
    NotAllowed,
    CancelArmed,
}

/// 画面キーボードの状態。
#[derive(Debug, Clone)]
pub struct SoftKeyboard {
    spec: SoftKeyboardFieldSpec,
    initial: Vec<char>,
    text: Vec<char>,
    caret: usize,
    mode: SoftKeyboardMode,
    row: usize,
    column: usize,
    cancel_armed: bool,
    notice: Option<SoftKeyboardNotice>,
    outcome: Option<SoftKeyboardOutcome>,
    finished: bool,
}

impl SoftKeyboard {
    /// 欄の仕様を受け取って開く。キャレットは末尾、カーソルは文字盤の左上。
    pub fn open(mut spec: SoftKeyboardFieldSpec) -> Self {
        spec.modes = normalized_modes(&spec.modes);
        let mut initial: Vec<char> = spec.initial_text.chars().collect();
        if let Some(max) = spec.max_chars {
            initial.truncate(max);
        }
        let mode =
            spec.initial_mode.filter(|mode| spec.modes.contains(mode)).unwrap_or(spec.modes[0]);
        let caret = initial.len();
        let mut keyboard = Self {
            spec,
            text: initial.clone(),
            initial,
            caret,
            mode,
            row: 0,
            column: 0,
            cancel_armed: false,
            notice: None,
            outcome: None,
            finished: false,
        };
        keyboard.snap_cursor();
        keyboard
    }

    pub fn spec(&self) -> &SoftKeyboardFieldSpec {
        &self.spec
    }

    /// 入力中の文字列（マスクしない）。
    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// 表示用の文字列。マスクする欄では [`MASK_CHAR`] を並べる。
    pub fn display_text(&self) -> String {
        if self.spec.masked {
            std::iter::repeat_n(MASK_CHAR, self.text.len()).collect()
        } else {
            self.text()
        }
    }

    /// 入力中の文字数。
    pub fn char_count(&self) -> usize {
        self.text.len()
    }

    /// キャレットの位置（文字数単位。0 は先頭）。
    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn mode(&self) -> SoftKeyboardMode {
        self.mode
    }

    /// 使える文字種（切替順）。
    pub fn modes(&self) -> &[SoftKeyboardMode] {
        &self.spec.modes
    }

    /// 文字盤のカーソル位置 `(row, column)`。
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.column)
    }

    /// カーソル位置の文字。
    pub fn selected_char(&self) -> Option<char> {
        self.mode.grid().get(self.row).and_then(|row| row[self.column])
    }

    pub fn notice(&self) -> Option<SoftKeyboardNotice> {
        self.notice
    }

    /// 確定・取消の後なら true。以降の操作は無視する。
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// 確定・取消の結果を 1 回だけ取り出す。
    pub fn take_outcome(&mut self) -> Option<SoftKeyboardOutcome> {
        self.outcome.take()
    }

    /// 操作を 1 つ適用する。
    pub fn apply(&mut self, action: SoftKeyboardAction) -> SoftKeyboardEvent {
        if self.finished {
            return SoftKeyboardEvent::Ignored;
        }
        if action != SoftKeyboardAction::Cancel {
            self.cancel_armed = false;
        }
        let event = self.apply_inner(action);
        self.notice = match event {
            SoftKeyboardEvent::Rejected(SoftKeyboardRejection::MaxLength) => {
                Some(SoftKeyboardNotice::MaxLength)
            }
            SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed) => {
                Some(SoftKeyboardNotice::NotAllowed)
            }
            SoftKeyboardEvent::CancelArmed => Some(SoftKeyboardNotice::CancelArmed),
            _ => None,
        };
        event
    }

    fn apply_inner(&mut self, action: SoftKeyboardAction) -> SoftKeyboardEvent {
        use SoftKeyboardAction as A;
        match action {
            A::CursorUp => self.move_vertical(false),
            A::CursorDown => self.move_vertical(true),
            A::CursorLeft => self.move_horizontal(false),
            A::CursorRight => self.move_horizontal(true),
            A::MoveCursorTo { row, column } => {
                let grid = self.mode.grid();
                if grid.get(row).and_then(|cells| cells.get(column)).copied().flatten().is_none() {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
                }
                self.row = row;
                self.column = column;
                SoftKeyboardEvent::Moved
            }
            A::InputSelected => match self.selected_char() {
                Some(ch) => self.insert(ch),
                None => SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing),
            },
            A::InputSpace => {
                if self.spec.allow_space {
                    self.insert(' ')
                } else {
                    SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed)
                }
            }
            A::Convert => self.convert_previous(),
            A::Backspace => {
                if self.caret == 0 {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
                }
                self.caret -= 1;
                self.text.remove(self.caret);
                SoftKeyboardEvent::TextChanged
            }
            A::ClearAll => {
                if self.text.is_empty() {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
                }
                self.text.clear();
                self.caret = 0;
                SoftKeyboardEvent::TextChanged
            }
            A::CaretLeft => {
                if self.caret == 0 {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
                }
                self.caret -= 1;
                SoftKeyboardEvent::Moved
            }
            A::CaretRight => {
                if self.caret >= self.text.len() {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
                }
                self.caret += 1;
                SoftKeyboardEvent::Moved
            }
            A::NextMode => self.step_mode(true),
            A::PreviousMode => self.step_mode(false),
            A::SetMode(mode) => {
                if !self.spec.modes.contains(&mode) {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed);
                }
                self.set_mode(mode)
            }
            A::Submit => {
                self.finished = true;
                self.outcome = Some(SoftKeyboardOutcome::Submitted(self.text()));
                SoftKeyboardEvent::Submitted
            }
            A::Cancel => {
                if self.text != self.initial && !self.cancel_armed {
                    self.cancel_armed = true;
                    return SoftKeyboardEvent::CancelArmed;
                }
                self.finished = true;
                self.outcome = Some(SoftKeyboardOutcome::Cancelled);
                SoftKeyboardEvent::Cancelled
            }
        }
    }

    fn insert(&mut self, ch: char) -> SoftKeyboardEvent {
        if !self.char_allowed(ch) {
            return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::NotAllowed);
        }
        if self.spec.max_chars.is_some_and(|max| self.text.len() >= max) {
            return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::MaxLength);
        }
        self.text.insert(self.caret, ch);
        self.caret += 1;
        SoftKeyboardEvent::TextChanged
    }

    fn convert_previous(&mut self) -> SoftKeyboardEvent {
        let Some(index) = self.caret.checked_sub(1) else {
            return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
        };
        let original = self.text[index];
        // 変換の輪の中に使えない文字がある場合（ASCII だけの欄など）は、使える文字まで進める。
        let mut candidate = original;
        loop {
            match convert_char(candidate) {
                Some(next) if next == original => {
                    return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
                }
                Some(next) if self.char_allowed(next) => {
                    self.text[index] = next;
                    return SoftKeyboardEvent::TextChanged;
                }
                Some(next) => candidate = next,
                None => return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing),
            }
        }
    }

    fn char_allowed(&self, ch: char) -> bool {
        if ch == ' ' {
            return self.spec.allow_space;
        }
        char_in_modes(ch, &self.spec.modes)
    }

    fn step_mode(&mut self, forward: bool) -> SoftKeyboardEvent {
        let modes = &self.spec.modes;
        if modes.len() <= 1 {
            return SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing);
        }
        let index = modes.iter().position(|&mode| mode == self.mode).unwrap_or(0);
        let next = if forward {
            (index + 1) % modes.len()
        } else {
            (index + modes.len() - 1) % modes.len()
        };
        self.set_mode(modes[next])
    }

    fn set_mode(&mut self, mode: SoftKeyboardMode) -> SoftKeyboardEvent {
        self.mode = mode;
        self.snap_cursor();
        SoftKeyboardEvent::Moved
    }

    /// 文字盤の範囲内へ収め、空きマスなら同じ行の右、次に同じ列の上へ寄せる。
    fn snap_cursor(&mut self) {
        let grid = self.mode.grid();
        self.row = self.row.min(grid.len() - 1);
        self.column = self.column.min(GRID_COLUMNS - 1);
        if self.selected_char().is_some() {
            return;
        }
        if let Some(column) = (self.column..GRID_COLUMNS)
            .chain((0..self.column).rev())
            .find(|&column| grid[self.row][column].is_some())
        {
            self.column = column;
            return;
        }
        if let Some(row) = (0..grid.len()).find(|&row| grid[row][self.column].is_some()) {
            self.row = row;
        }
    }

    fn move_horizontal(&mut self, forward: bool) -> SoftKeyboardEvent {
        let row = &self.mode.grid()[self.row];
        let mut column = self.column;
        for _ in 0..GRID_COLUMNS {
            column = if forward {
                (column + 1) % GRID_COLUMNS
            } else {
                (column + GRID_COLUMNS - 1) % GRID_COLUMNS
            };
            if row[column].is_some() {
                self.column = column;
                return SoftKeyboardEvent::Moved;
            }
        }
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    }

    fn move_vertical(&mut self, forward: bool) -> SoftKeyboardEvent {
        let grid = self.mode.grid();
        let rows = grid.len();
        let mut row = self.row;
        for _ in 0..rows {
            row = if forward { (row + 1) % rows } else { (row + rows - 1) % rows };
            if grid[row][self.column].is_some() {
                if row == self.row {
                    break;
                }
                self.row = row;
                return SoftKeyboardEvent::Moved;
            }
        }
        SoftKeyboardEvent::Rejected(SoftKeyboardRejection::Nothing)
    }
}

fn normalized_modes(modes: &[SoftKeyboardMode]) -> Vec<SoftKeyboardMode> {
    let normalized: Vec<_> =
        SoftKeyboardMode::ALL.into_iter().filter(|mode| modes.contains(mode)).collect();
    if normalized.is_empty() { SoftKeyboardMode::ALL.to_vec() } else { normalized }
}
