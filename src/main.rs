use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use tetris_tui::game::Game;
use tetris_tui::input::{self, Outcome};

/// 渲染帧间隔（~30fps）：一切动画由固定节奏驱动。
const FRAME: Duration = Duration::from_millis(33);

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let res = run(&mut terminal);
    ratatui::restore();
    res
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut game = Game::new(); // 新局即 READY 态，空格开始
    let mut last = Instant::now(); // 重力计时
    loop {
        if !game.paused && game.clear_anim_done() {
            game.finish_clear();
            last = Instant::now();
        }
        game.tick_effects();
        terminal.draw(|f| tetris_tui::ui::draw(f, &game))?;

        // 帧预算内排空输入，渲染保持固定 ~30fps 节奏
        let deadline = Instant::now() + FRAME;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if !event::poll(left)? {
                break;
            }
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match input::handle_key(&mut game, k.code, k.modifiers) {
                        Outcome::Quit => return Ok(()),
                        Outcome::Reset => last = Instant::now(),
                        Outcome::Nop => {}
                    }
                }
            }
        }

        if !game.paused
            && !game.over
            && !game.is_clearing()
            && last.elapsed() >= game.drop_interval()
        {
            game.tick();
            last = Instant::now();
        }
    }
}
