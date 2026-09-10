//! 按键映射：h/l 左右 · k 变形（z 反向）· j 硬降落底 · J 软降一格 · 空格 暂停/开始 · r 重开 · q 退出
use crate::game::Game;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

/// 一次按键的处理结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Nop,
    /// 纵向位移/状态切换：主循环应重置重力计时
    Reset,
    Quit,
}

pub fn handle_key(game: &mut Game, code: KeyCode, mods: KeyModifiers) -> Outcome {
    use KeyCode::*;
    let ctrl_c = mods.contains(KeyModifiers::CONTROL) && matches!(code, Char('c'));
    if matches!(code, Char('q') | Esc) || ctrl_c {
        return Outcome::Quit;
    }
    match code {
        // 空格在结束态不生效，只能 r 重开
        Char(' ') => {
            if !game.over {
                game.paused = !game.paused;
            }
            Outcome::Reset
        }
        Char('r') => {
            *game = Game::new();
            Outcome::Reset
        }
        _ if game.paused || game.over => Outcome::Nop,
        Char('h') => {
            game.try_move(-1, 0);
            Outcome::Nop
        }
        Char('l') => {
            game.try_move(1, 0);
            Outcome::Nop
        }
        Char('k') => {
            game.rotate(1);
            Outcome::Nop
        }
        Char('z') => {
            game.rotate(3);
            Outcome::Nop
        }
        Char('j') => {
            game.hard_drop();
            Outcome::Reset
        }
        Char('J') => {
            if game.try_move(0, 1) {
                game.score += 1;
            }
            Outcome::Reset
        }
        Char('c') => {
            game.hold();
            Outcome::Reset
        }
        _ => Outcome::Nop,
    }
}
