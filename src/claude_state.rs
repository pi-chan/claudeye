use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, PartialEq, Clone)]
pub enum ClaudeState {
    Working,
    WaitingForApproval,
    /// 将来の拡張用（現在は detect_state() から返されないが UI で表示定義済み）
    #[allow(dead_code)]
    WaitingForAnswer,
    Idle,
    /// 将来の拡張用（現在は detect_state() から返されないが UI で表示定義済み）
    #[allow(dead_code)]
    NotRunning,
}

/// 状態判定に使用する末尾の最大行数
const LAST_LINES_COUNT: usize = 30;

/// tcmux の parseClaudeStatus を Rust に移植したメイン判定関数。
/// capture-pane の出力文字列を受け取り、Claude Code の状態を返す。
pub fn detect_state(content: &str) -> ClaudeState {
    let lines: Vec<&str> = content.split('\n').collect();
    let last_lines = last_non_empty_lines(&lines, LAST_LINES_COUNT);
    let combined = last_lines.join("\n");

    // Running チェック（最優先）
    // Format 1: (esc to interrupt · 1m 45s · ...) — time after middle dot
    if running_pattern().is_match(&combined) {
        return ClaudeState::Working;
    }

    // Format 2: (1m 52s · ...) — time at beginning of parentheses
    if running_pattern_time_first().is_match(&combined) {
        return ClaudeState::Working;
    }

    // Fallback: "(esc to interrupt)" or "(ctrl+c to interrupt)" — no time
    if running_fallback_pattern().is_match(&combined) {
        return ClaudeState::Working;
    }

    // "· esc to interrupt" がステータス行末尾にある場合（例: "4 files +20 -0 · esc to interrupt"）
    if esc_to_interrupt_end_pattern().is_match(&combined) {
        return ClaudeState::Working;
    }

    // 汎用ステータス行パターン: "✻ Doing… (" のような行頭シンボル + 動詞 + … + (
    // タイマー未表示の初期思考段階（"(thinking)", "(thought for 2s)" 等）を捕捉する
    if running_generic_pattern().is_match(&combined) {
        return ClaudeState::Working;
    }

    // Idle チェック: 末尾の意味のある行が ❯ プロンプトであれば Idle
    // isClaudePromptLine は生の lines 配列で判定（footer/separator をスキップ）
    if is_claude_prompt_line(&lines) {
        return ClaudeState::Idle;
    }

    // Waiting チェック: 許可・確認ダイアログのパターン
    for &pattern in WAITING_PATTERNS.iter() {
        if combined.contains(pattern) {
            return ClaudeState::WaitingForApproval;
        }
    }

    // インタビューモード: "Enter to select · ↑/↓ to navigate · Esc to cancel"
    if interview_pattern().is_match(&combined) {
        return ClaudeState::WaitingForApproval;
    }

    // 番号付き選択メニュー: "❯ 1. Yes" 等
    if selection_menu_pattern().is_match(&combined) {
        return ClaudeState::WaitingForApproval;
    }

    // Idle フォールバック: combined に ❯ があれば Idle
    if idle_pattern().is_match(&combined) {
        return ClaudeState::Idle;
    }

    // Unknown は使わない → Idle
    ClaudeState::Idle
}

// ─── Waiting パターン文字列一覧（tcmux claudeWaitingPatterns に対応） ───
static WAITING_PATTERNS: &[&str] = &[
    "Yes, allow once",
    "Yes, allow always",
    "Allow once",
    "Allow always",
    "❯ Yes",
    "❯ No",
    "Do you trust",
    "Run this command?",
    "Allow this MCP server",
    "Continue?",
    "Proceed?",
    "Do you want to proceed?",
    "(Y/n)",
    "(y/N)",
    "[Y/n]",
    "[y/N]",
];

// ─── 正規表現（OnceLock で遅延初期化） ───

fn running_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // Format 1: (esc to interrupt · 1m 45s · ...) — time after middle dot
        Regex::new(r"(?m)^[✢✽✶✻·]\s+.+?…?\s*\([^)]*·\s*((?:\d+[smh]\s*)+)").unwrap()
    })
}

fn running_pattern_time_first() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // Format 2: (1m 52s · ...) — time at beginning of parentheses
        Regex::new(r"(?m)^[✢✽✶✻·]\s+.+?…?\s*\(((?:\d+[smh]\s*)+)\s*·").unwrap()
    })
}

fn running_fallback_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // "(esc to interrupt)" または "(ctrl+c to interrupt)" — 時刻なし
        Regex::new(r"(?m)^[✢✽✶✻·]\s+.+?…?\s*\((esc|ctrl\+c) to interrupt").unwrap()
    })
}

fn esc_to_interrupt_end_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // "4 files +20 -0 · esc to interrupt" のようにステータス行末尾にある場合
        Regex::new(r"(?m)·\s*esc to interrupt(\s|·|$)").unwrap()
    })
}

fn running_generic_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // "✻ Doing… (thinking)" や "✻ Thinking…" のような汎用ステータス行
        // 行頭シンボル + 動詞 + … があれば進行中と判定（括弧は不要）
        // インデントされた行（quoted text）は ^ で除外される
        Regex::new(r"(?m)^[✢✽✶✻·]\s+.+?…").unwrap()
    })
}

fn selection_menu_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // 番号付き選択メニュー: "❯ 1. Yes" 等
        Regex::new(r"❯\s+\d+\.").unwrap()
    })
}

fn file_changes_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // "4 files +42 -0", "1 file +10 -5" 等のファイル変更行
        Regex::new(r"^\s*\d+\s+files?\s+[+\-]").unwrap()
    })
}

fn idle_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // Idle フォールバック: 行頭の ❯
        Regex::new(r"(?m)^\s*❯").unwrap()
    })
}

fn interview_pattern() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| {
        // インタビューモード
        Regex::new(r"Enter to select.*↑/↓ to navigate.*Esc to cancel").unwrap()
    })
}

// ─── ヘルパー関数 ───

/// 末尾から最大 n 個の非空・非セパレータ行を元の順序で返す。
/// Go の lastNonEmptyLines に対応。
fn last_non_empty_lines<'a>(lines: &[&'a str], n: usize) -> Vec<&'a str> {
    let mut result = Vec::new();
    for &line in lines.iter().rev() {
        if result.len() >= n {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_separator_line(trimmed) {
            continue;
        }
        result.push(line);
    }
    result.reverse();
    result
}

/// 行が Box Drawing 文字（U+2500〜U+257F）のみで構成されているか判定。
/// 空文字列は true を返す（Go の isSeparatorLine と同じ挙動）。
fn is_separator_line(line: &str) -> bool {
    line.chars().all(|c| ('\u{2500}'..='\u{257F}').contains(&c))
}

/// 末尾の意味のある行（空・セパレータ・フッターを除く）が ❯ プロンプトかを判定。
/// Go の isClaudePromptLine に対応。
fn is_claude_prompt_line(lines: &[&str]) -> bool {
    let sel = selection_menu_pattern();
    let file_changes = file_changes_pattern();

    for &line in lines.iter().rev() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }
        if is_separator_line(trimmed) {
            continue;
        }

        // フッター行をスキップ
        if trimmed.contains("? for shortcuts")
            || trimmed.contains("ctrl+")
            || trimmed.contains("shift+")
            || file_changes.is_match(trimmed)
        {
            continue;
        }

        // ❯ で始まる行がプロンプト（番号付き選択メニューや待機パターンは除外）
        if trimmed.starts_with('❯') {
            if sel.is_match(trimmed) {
                return false; // ❯ 1. Yes style 選択メニュー
            }
            // 「❯ Yes」「❯ No」など ❯ で始まる待機パターンはプロンプトと見なさない
            for &pattern in WAITING_PATTERNS.iter() {
                if pattern.starts_with('❯') && trimmed == pattern {
                    return false;
                }
            }
            return true;
        }

        // ❯ 以外の意味ある行が見つかった → プロンプトではない
        return false;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_line_box_drawing() {
        assert!(is_separator_line("─────────────────"));
    }

    #[test]
    fn separator_line_double() {
        assert!(is_separator_line("═════════════════"));
    }

    #[test]
    fn separator_line_mixed_box_drawing() {
        assert!(is_separator_line("─═─═─═─═─"));
    }

    #[test]
    fn separator_line_text_content() {
        assert!(!is_separator_line("Some text"));
    }

    #[test]
    fn separator_line_mixed_with_text() {
        assert!(!is_separator_line("───text───"));
    }

    #[test]
    fn separator_line_empty() {
        assert!(is_separator_line(""));
    }
}
