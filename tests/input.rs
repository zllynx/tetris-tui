use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tetris_tui::game::Game;
use tetris_tui::input::{handle_key, Outcome};

fn key(g: &mut Game, c: char) -> Outcome {
    handle_key(g, KeyCode::Char(c), KeyModifiers::NONE)
}

fn space(g: &mut Game) -> Outcome {
    handle_key(g, KeyCode::Char(' '), KeyModifiers::NONE)
}

/// 新局是 READY 态；空格负责 开始 ⇄ 暂停 切换。
#[test]
fn space_toggles_start_and_pause() {
    let mut g = Game::new();
    assert!(g.paused, "新局应处于 READY（暂停）态");
    space(&mut g);
    assert!(!g.paused);
    space(&mut g);
    assert!(g.paused);
}

/// 暂停中移动/变形/软降/硬降全部失效。
#[test]
fn movement_dead_while_paused() {
    let mut g = Game::new();
    space(&mut g); // 开始
    space(&mut g); // 暂停
    let (x, r, y, s) = (g.piece.x, g.piece.rot, g.piece.y, g.score);
    key(&mut g, 'h');
    key(&mut g, 'l');
    key(&mut g, 'k');
    key(&mut g, 'j');
    key(&mut g, 'J');
    assert_eq!((g.piece.x, g.piece.rot, g.piece.y, g.score), (x, r, y, s));
}

#[test]
fn hl_move_k_rotate() {
    let mut g = Game::new();
    space(&mut g);
    let x = g.piece.x;
    key(&mut g, 'h');
    assert_eq!(g.piece.x, x - 1);
    key(&mut g, 'l');
    assert_eq!(g.piece.x, x);

    let r = g.piece.rot;
    key(&mut g, 'k');
    assert_eq!(g.piece.rot, (r + 1) % 4);

    assert_eq!(key(&mut g, 'q'), Outcome::Quit);
}

/// Shift+J 软降一格并计 1 分。
#[test]
fn shift_j_soft_drops_one_cell() {
    let mut g = Game::new();
    space(&mut g);
    let s = g.score;
    let out = handle_key(&mut g, KeyCode::Char('J'), KeyModifiers::SHIFT);
    assert_eq!(out, Outcome::Reset, "软降要重置重力计时");
    assert_eq!(g.piece.y, 1);
    assert_eq!(g.piece.x, g.piece.x);
    assert_eq!(g.score, s + 1);
}

/// j 硬降：直接落到最下面，锁进底部并立即换新块。
#[test]
fn j_hard_drops_to_bottom() {
    let mut g = Game::new();
    space(&mut g);
    let out = key(&mut g, 'j');
    assert_eq!(out, Outcome::Reset);
    assert!(g.board[19].iter().any(|c| c.is_some()));
    assert!(!g.over);
    assert_eq!(g.piece.y, 0, "锁定后应生成下一块");
    assert!(g.score > 0, "硬降按距离得分");
}

/// 结束态空格不再恢复，只能 r 重开。
#[test]
fn space_ignored_after_game_over() {
    let mut g = Game::new();
    space(&mut g);
    assert!(!g.paused);
    g.over = true;
    space(&mut g);
    assert!(!g.paused);
}

#[test]
fn r_restarts_to_ready() {
    let mut g = Game::new();
    space(&mut g);
    key(&mut g, 'J');
    assert!(g.score > 0);
    key(&mut g, 'r');
    assert_eq!(g.score, 0);
    assert!(g.paused, "重开回到 READY 态");
    assert!(g.board.iter().all(|row| row.iter().all(|c| c.is_none())));
}
