use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use tetris_tui::game::{Game, Shape, W};
use tetris_tui::ui::draw;

fn white_cells(backend: &TestBackend) -> usize {
    backend
        .buffer()
        .content()
        .iter()
        .filter(|c| c.fg == Color::White)
        .count()
}

/// 消行动画：满行整行以白色闪烁渲染（覆盖该行所有列，包括原本为空的格）。
#[test]
fn clearing_rows_render_white_flash() {
    let mut g = Game::new();
    for x in 0..W {
        if x != 4 && x != 5 {
            g.board[19][x] = Some(Shape::Z);
        }
    }
    g.spawn(Shape::O);
    g.hard_drop(); // O 补满第 19 行 → 进入消行动画
    assert!(g.is_clearing());

    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    term.draw(|f| draw(f, &g)).unwrap();
    // 一行 = 10 逻辑格 × 2 终端列；游戏配色没有纯白，白色块只能来自闪烁层
    assert!(
        white_cells(term.backend()) >= 20,
        "消行整行应以白色闪烁"
    );

    g.finish_clear();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    term.draw(|f| draw(f, &g)).unwrap();
    assert_eq!(white_cells(term.backend()), 0, "动画结束后不应再有白色闪烁行");
}


/// 砖块使用 RGB 斜面色板渲染，且 tile 是双色调（fg≠bg），不再是单色实心块。
#[test]
fn bricks_render_truecolor_bevel() {
    let mut g = Game::new();
    g.board[19][0] = Some(Shape::Z);
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    term.draw(|f| draw(f, &g)).unwrap();
    let cells = term.backend().buffer().content();
    let rgb_fg = cells
        .iter()
        .filter(|c| matches!(c.fg, Color::Rgb(..)))
        .count();
    assert!(rgb_fg >= 2, "砖块 tile 应带 RGB 前景色: {rgb_fg}");
    let two_tone = cells
        .iter()
        .filter(|c| matches!(c.fg, Color::Rgb(..)) && c.fg != c.bg)
        .count();
    assert!(two_tone >= 1, "斜面 tile 应是 fg/bg 双色调: {two_tone}");
}