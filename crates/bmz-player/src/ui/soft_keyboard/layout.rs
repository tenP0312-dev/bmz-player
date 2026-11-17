//! 画面キーボードの文字種ごとの文字盤と、直前の文字の変換規則。

/// 文字盤の 1 マス。`None` は空きマスで、カーソルは止まらない。
pub type SoftKeyboardCell = Option<char>;

/// 画面キーボードの文字種（ページ）。並び順は文字種の切替順と同じ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SoftKeyboardMode {
    /// ひらがな（50 音・ん・長音）。濁点・半濁点・小書きは変換で付ける。
    Hiragana,
    /// カタカナ。並びはひらがなと同じ。
    Katakana,
    /// 英小文字と数字、URL・メールで使う `- _ . @`。
    AlphaLower,
    /// 英大文字と数字、URL・メールで使う `- _ . @`。
    AlphaUpper,
    /// ASCII の記号と空白。
    Symbol,
    /// 全角の記号（曲名でよく使う ☆ ★ ♪ † など）。
    WideSymbol,
}

impl SoftKeyboardMode {
    /// すべての文字種を切替順に並べたもの。
    pub const ALL: [Self; 6] = [
        Self::Hiragana,
        Self::Katakana,
        Self::AlphaLower,
        Self::AlphaUpper,
        Self::Symbol,
        Self::WideSymbol,
    ];

    /// ASCII だけの欄（URL・パス・メール・パスワード）で使える文字種。
    pub const ASCII: [Self; 3] = [Self::AlphaLower, Self::AlphaUpper, Self::Symbol];

    /// この文字種の文字がすべて ASCII なら true。
    pub const fn is_ascii(self) -> bool {
        matches!(self, Self::AlphaLower | Self::AlphaUpper | Self::Symbol)
    }

    /// 文字種のタブに表示する翻訳キー。
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Hiragana => "soft-keyboard-mode-hiragana",
            Self::Katakana => "soft-keyboard-mode-katakana",
            Self::AlphaLower => "soft-keyboard-mode-alpha-lower",
            Self::AlphaUpper => "soft-keyboard-mode-alpha-upper",
            Self::Symbol => "soft-keyboard-mode-symbol",
            Self::WideSymbol => "soft-keyboard-mode-wide-symbol",
        }
    }

    /// 文字盤（行の配列）。どの行も [`GRID_COLUMNS`] 列。
    pub fn grid(self) -> &'static [[SoftKeyboardCell; GRID_COLUMNS]] {
        match self {
            Self::Hiragana => &HIRAGANA,
            Self::Katakana => &KATAKANA,
            Self::AlphaLower => &ALPHA_LOWER,
            Self::AlphaUpper => &ALPHA_UPPER,
            Self::Symbol => &SYMBOL,
            Self::WideSymbol => &WIDE_SYMBOL,
        }
    }
}

/// どの文字盤も同じ列数にして、文字種を切り替えてもカーソルの列を保てるようにする。
pub const GRID_COLUMNS: usize = 11;

const fn c(ch: char) -> SoftKeyboardCell {
    Some(ch)
}

const N: SoftKeyboardCell = None;

/// 50 音表を左から あ か さ … と並べ、縦を あ い う え お の段にする。
/// 右端の列は ん・を・長音。
#[rustfmt::skip]
const HIRAGANA: [[SoftKeyboardCell; GRID_COLUMNS]; 5] = [
    [c('あ'), c('か'), c('さ'), c('た'), c('な'), c('は'), c('ま'), c('や'), c('ら'), c('わ'), c('ん')],
    [c('い'), c('き'), c('し'), c('ち'), c('に'), c('ひ'), c('み'), N, c('り'), N, c('ー')],
    [c('う'), c('く'), c('す'), c('つ'), c('ぬ'), c('ふ'), c('む'), c('ゆ'), c('る'), c('を'), N],
    [c('え'), c('け'), c('せ'), c('て'), c('ね'), c('へ'), c('め'), N, c('れ'), N, N],
    [c('お'), c('こ'), c('そ'), c('と'), c('の'), c('ほ'), c('も'), c('よ'), c('ろ'), N, N],
];

#[rustfmt::skip]
const KATAKANA: [[SoftKeyboardCell; GRID_COLUMNS]; 5] = [
    [c('ア'), c('カ'), c('サ'), c('タ'), c('ナ'), c('ハ'), c('マ'), c('ヤ'), c('ラ'), c('ワ'), c('ン')],
    [c('イ'), c('キ'), c('シ'), c('チ'), c('ニ'), c('ヒ'), c('ミ'), N, c('リ'), N, c('ー')],
    [c('ウ'), c('ク'), c('ス'), c('ツ'), c('ヌ'), c('フ'), c('ム'), c('ユ'), c('ル'), c('ヲ'), N],
    [c('エ'), c('ケ'), c('セ'), c('テ'), c('ネ'), c('ヘ'), c('メ'), N, c('レ'), N, N],
    [c('オ'), c('コ'), c('ソ'), c('ト'), c('ノ'), c('ホ'), c('モ'), c('ヨ'), c('ロ'), N, N],
];

#[rustfmt::skip]
const ALPHA_LOWER: [[SoftKeyboardCell; GRID_COLUMNS]; 4] = [
    [c('1'), c('2'), c('3'), c('4'), c('5'), c('6'), c('7'), c('8'), c('9'), c('0'), N],
    [c('a'), c('b'), c('c'), c('d'), c('e'), c('f'), c('g'), c('h'), c('i'), c('j'), c('-')],
    [c('k'), c('l'), c('m'), c('n'), c('o'), c('p'), c('q'), c('r'), c('s'), c('t'), c('_')],
    [c('u'), c('v'), c('w'), c('x'), c('y'), c('z'), c('.'), c('@'), c('/'), c(':'), N],
];

#[rustfmt::skip]
const ALPHA_UPPER: [[SoftKeyboardCell; GRID_COLUMNS]; 4] = [
    [c('1'), c('2'), c('3'), c('4'), c('5'), c('6'), c('7'), c('8'), c('9'), c('0'), N],
    [c('A'), c('B'), c('C'), c('D'), c('E'), c('F'), c('G'), c('H'), c('I'), c('J'), c('-')],
    [c('K'), c('L'), c('M'), c('N'), c('O'), c('P'), c('Q'), c('R'), c('S'), c('T'), c('_')],
    [c('U'), c('V'), c('W'), c('X'), c('Y'), c('Z'), c('.'), c('@'), c('/'), c(':'), N],
];

/// ASCII の記号 32 種と空白。
#[rustfmt::skip]
const SYMBOL: [[SoftKeyboardCell; GRID_COLUMNS]; 3] = [
    [c('!'), c('"'), c('#'), c('$'), c('%'), c('&'), c('\''), c('('), c(')'), c('*'), c('+')],
    [c(','), c('-'), c('.'), c('/'), c(':'), c(';'), c('<'), c('='), c('>'), c('?'), c('@')],
    [c('['), c('\\'), c(']'), c('^'), c('_'), c('`'), c('{'), c('|'), c('}'), c('~'), c(' ')],
];

#[rustfmt::skip]
const WIDE_SYMBOL: [[SoftKeyboardCell; GRID_COLUMNS]; 3] = [
    [c('、'), c('。'), c('・'), c('ー'), c('～'), c('…'), c('「'), c('」'), c('『'), c('』'), c('　')],
    [c('！'), c('？'), c('（'), c('）'), c('＆'), c('＃'), c('＋'), c('＝'), c('／'), c('：'), c('※')],
    [c('☆'), c('★'), c('♪'), c('†'), c('○'), c('●'), c('◇'), c('◆'), c('□'), c('■'), c('△')],
];

/// 直前の文字の変換順。1 回押すごとに右隣へ進み、末尾の次は先頭へ戻る。
/// 濁点・半濁点・小書きの区別はせず、その文字で作れる形を順に並べる。
const CONVERSION_CYCLES: &[&[char]] = &[
    // ひらがな：小書き
    &['あ', 'ぁ'],
    &['い', 'ぃ'],
    &['え', 'ぇ'],
    &['お', 'ぉ'],
    &['や', 'ゃ'],
    &['ゆ', 'ゅ'],
    &['よ', 'ょ'],
    &['わ', 'ゎ'],
    // ひらがな：小書きと濁点の両方
    &['う', 'ぅ', 'ゔ'],
    &['つ', 'っ', 'づ'],
    // ひらがな：濁点
    &['か', 'が'],
    &['き', 'ぎ'],
    &['く', 'ぐ'],
    &['け', 'げ'],
    &['こ', 'ご'],
    &['さ', 'ざ'],
    &['し', 'じ'],
    &['す', 'ず'],
    &['せ', 'ぜ'],
    &['そ', 'ぞ'],
    &['た', 'だ'],
    &['ち', 'ぢ'],
    &['て', 'で'],
    &['と', 'ど'],
    // ひらがな：濁点と半濁点
    &['は', 'ば', 'ぱ'],
    &['ひ', 'び', 'ぴ'],
    &['ふ', 'ぶ', 'ぷ'],
    &['へ', 'べ', 'ぺ'],
    &['ほ', 'ぼ', 'ぽ'],
    // カタカナ
    &['ア', 'ァ'],
    &['イ', 'ィ'],
    &['エ', 'ェ'],
    &['オ', 'ォ'],
    &['ヤ', 'ャ'],
    &['ユ', 'ュ'],
    &['ヨ', 'ョ'],
    &['ワ', 'ヮ'],
    &['ウ', 'ゥ', 'ヴ'],
    &['ツ', 'ッ', 'ヅ'],
    &['カ', 'ガ', 'ヵ'],
    &['キ', 'ギ'],
    &['ク', 'グ'],
    &['ケ', 'ゲ', 'ヶ'],
    &['コ', 'ゴ'],
    &['サ', 'ザ'],
    &['シ', 'ジ'],
    &['ス', 'ズ'],
    &['セ', 'ゼ'],
    &['ソ', 'ゾ'],
    &['タ', 'ダ'],
    &['チ', 'ヂ'],
    &['テ', 'デ'],
    &['ト', 'ド'],
    &['ハ', 'バ', 'パ'],
    &['ヒ', 'ビ', 'ピ'],
    &['フ', 'ブ', 'プ'],
    &['ヘ', 'ベ', 'ペ'],
    &['ホ', 'ボ', 'ポ'],
];

/// `ch` を変換した次の文字。変換できない文字は `None`。
/// 英字は大文字と小文字を入れ替える。
pub fn convert_char(ch: char) -> Option<char> {
    if ch.is_ascii_lowercase() {
        return Some(ch.to_ascii_uppercase());
    }
    if ch.is_ascii_uppercase() {
        return Some(ch.to_ascii_lowercase());
    }
    CONVERSION_CYCLES.iter().find_map(|cycle| {
        let index = cycle.iter().position(|&candidate| candidate == ch)?;
        Some(cycle[(index + 1) % cycle.len()])
    })
}

/// `ch` がどれかの文字種の文字盤にあるか、変換で作れる文字なら true。
pub fn char_in_modes(ch: char, modes: &[SoftKeyboardMode]) -> bool {
    modes.iter().any(|mode| {
        mode.grid().iter().flatten().any(|cell| {
            cell.is_some_and(|base| base == ch || conversion_family(base).contains(&ch))
        })
    })
}

fn conversion_family(ch: char) -> &'static [char] {
    CONVERSION_CYCLES.iter().find(|cycle| cycle.contains(&ch)).copied().unwrap_or(&[])
}
