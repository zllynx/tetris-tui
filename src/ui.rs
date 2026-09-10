use crate::game::{
    Game, Shape, H, W, CLEAR_ANIM, LOCK_FLASH, POPUP_TIME, SHAKE_TIME, TRAIL_TIME,
};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::{Frame, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

use Constraint::{Length, Min};

/// 像素分辨率：每逻辑格 4×2 像素（每字符 tile 2×2），棋盘共 40×40 像素。
const CELL_W: usize = 4;
const CELL_H: usize = 2;
const PW: usize = W * CELL_W;
const PH: usize = H * CELL_H;

/// 像素画布：None = 透明（透出终端背景）。
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<Option<Color>>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![None; w * h],
        }
    }

    fn set(&mut self, x: usize, y: usize, c: Option<Color>) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = c;
        }
    }

    fn get(&self, x: usize, y: usize) -> Option<Color> {
        self.px.get(y * self.w + x).copied().flatten()
    }

    /// 把画布落到缓冲区：每 2×2 像素编码为一个 quadrant 字符（fg=亮、bg=暗）。
    fn blit(&self, buf: &mut Buffer, ox: u16, oy: u16) {
        for ty in 0..self.h / 2 {
            for tx in 0..self.w / 2 {
                let tl = self.get(tx * 2, ty * 2);
                let tr = self.get(tx * 2 + 1, ty * 2);
                let bl = self.get(tx * 2, ty * 2 + 1);
                let br = self.get(tx * 2 + 1, ty * 2 + 1);
                let (sym, fg, bg) = tile(tl, tr, bl, br);
                let (x, y) = (ox + tx as u16, oy + ty as u16);
                if x < buf.area.width && y < buf.area.height {
                    buf[(x, y)].set_symbol(sym).set_fg(fg).set_bg(bg);
                }
            }
        }
    }
}

/// 2×2 像素 tile → 一个字符：同色实心，异色取 quadrant（fg=亮像素色）。
fn tile(
    tl: Option<Color>,
    tr: Option<Color>,
    bl: Option<Color>,
    br: Option<Color>,
) -> (&'static str, Color, Color) {
    let fg = tl.or(tr).or(bl).or(br).unwrap_or(Color::Reset);
    let on = |p: Option<Color>| p == Some(fg);
    let m = (on(tl), on(tr), on(bl), on(br));
    let bg = [tl, tr, bl, br]
        .into_iter()
        .find(|p| *p != Some(fg))
        .map(|p| p.unwrap_or(Color::Reset))
        .unwrap_or(fg);
    let sym = match m {
        (true, true, true, true) => "█",
        (true, true, false, false) => "▀",
        (false, false, true, true) => "▄",
        (true, false, true, false) => "▌",
        (false, true, false, true) => "▐",
        (true, false, false, false) => "▘",
        (false, true, false, false) => "▝",
        (false, false, true, false) => "▖",
        (false, false, false, true) => "▗",
        (true, false, false, true) => "▚",
        (false, true, true, false) => "▞",
        (true, true, false, true) => "▛",
        (true, true, true, false) => "▜",
        (true, false, true, true) => "▙",
        (false, true, true, true) => "▟",
        (false, false, false, false) => " ",
    };
    (sym, fg, bg)
}

/// 经典七色基色（RGB）；亮/暗档由 mix 派生用于斜面。
fn base(s: Shape) -> (u8, u8, u8) {
    match s {
        Shape::I => (70, 215, 255),
        Shape::O => (255, 199, 44),
        Shape::T => (186, 85, 255),
        Shape::S => (70, 230, 110),
        Shape::Z => (255, 75, 85),
        Shape::J => (75, 125, 255),
        Shape::L => (255, 145, 50),
    }
}

fn mix((r, g, b): (u8, u8, u8), to: (u8, u8, u8), k: f32) -> Color {
    let ch = |a: u8, t: u8| (a as f32 + (t as f32 - a as f32) * k).round() as u8;
    Color::Rgb(ch(r, to.0), ch(g, to.1), ch(b, to.2))
}

/// 斜面砖块：4×2 像素，左上高光 → 右下阴影的对角渐变。
fn brick(cv: &mut Canvas, cx: usize, cy: usize, s: Shape) {
    let b = base(s);
    let light = mix(b, (255, 255, 255), 0.55);
    let dark = mix(b, (0, 0, 0), 0.5);
    let mid = Color::Rgb(b.0, b.1, b.2);
    let (x0, y0) = (cx * CELL_W, cy * CELL_H);
    for (i, c) in [light, light, mid, mid].into_iter().enumerate() {
        cv.set(x0 + i, y0, Some(c));
    }
    for (i, c) in [mid, mid, dark, dark].into_iter().enumerate() {
        cv.set(x0 + i, y0 + 1, Some(c));
    }
}

/// ghost 方块：50% 棋盘抖动点阵。
fn checker(cv: &mut Canvas, cx: usize, cy: usize) {
    for dy in 0..CELL_H {
        for dx in 0..CELL_W {
            let (x, y) = (cx * CELL_W + dx, cy * CELL_H + dy);
            if (x + y) % 2 == 0 {
                cv.set(x, y, Some(Color::DarkGray));
            }
        }
    }
}

fn rgb_of(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (128, 128, 128),
    }
}

/// 把已画像素向目标色混合 k（白闪/渐隐用）。
fn fade_px(cv: &mut Canvas, x: usize, y: usize, to: (u8, u8, u8), k: f32) {
    if let Some(c) = cv.get(x, y) {
        cv.set(x, y, Some(mix(rgb_of(c), to, k)));
    }
}

/// 直接往缓冲区写一串字符（飘分用）。
fn put_text(buf: &mut Buffer, x: i32, y: i32, s: &str, color: Color) {
    if y < 0 || y >= buf.area.height as i32 {
        return;
    }
    for (i, ch) in s.chars().enumerate() {
        let cx = x + i as i32;
        if (0..buf.area.width as i32).contains(&cx) {
            buf[(cx as u16, y as u16)].set_symbol(&ch.to_string()).set_fg(color);
        }
    }
}

/// 整体布局：垂直居中一条 24 行的带，带内左边棋盘、右边信息面板。
pub fn draw(f: &mut Frame, game: &Game) {
    let rows = Layout::vertical([Min(0), Length(24), Length(1), Min(0)]).split(f.area());
    let cols =
        Layout::horizontal([Min(0), Length(22), Length(2), Length(20), Min(0)]).split(rows[1]);
    let board_area = Layout::vertical([Min(0), Length(22), Min(0)]).split(cols[1])[1];
    draw_board(f, game, board_area);
    draw_panel(f, game, cols[3]);
    draw_help(f, rows[2]);
}

fn draw_board(f: &mut Frame, game: &Game, area: Rect) {
    f.render_widget(
        Block::bordered()
            .title(" TETRIS ")
            .border_style(Style::new().dark_gray()),
        area,
    );
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let mut cv = Canvas::new(PW, PH);

    for y in 0..H {
        for x in 0..W {
            if let Some(s) = game.board[y][x] {
                brick(&mut cv, x, y, s);
            }
        }
    }

    // 硬降拖影：方块原色竖向渐隐，越接近落点越亮
    if let Some(t) = &game.trail {
        let k = (1.0 - t.born.elapsed().as_secs_f32() / TRAIL_TIME.as_secs_f32()).clamp(0.0, 1.0);
        let light = mix(base(t.shape), (255, 255, 255), 0.55);
        for &(x, y0, y1) in &t.cols {
            for cy in y0.max(0)..y1.max(0) {
                let span = (y1 - y0).max(1) as f32;
                let near = 1.0 - (y1 - cy) as f32 / span;
                let a = k * (0.25 + 0.75 * near);
                let c = mix(rgb_of(light), (0, 0, 0), 1.0 - a);
                let px = x as usize * CELL_W;
                for dx in 1..CELL_W - 1 {
                    cv.set(px + dx, cy as usize * CELL_H, Some(c));
                    cv.set(px + dx, cy as usize * CELL_H + 1, Some(c));
                }
            }
        }
    }

    if !game.over && !game.is_clearing() {
        let p = game.piece;
        let gy = game.ghost_y();
        for (cx, cy) in p.shape.cells(p.rot) {
            let gx = p.x + cx;
            let y = gy + cy;
            if y >= 0 && (y as usize) < H && game.board[y as usize][gx as usize].is_none() {
                checker(&mut cv, gx as usize, y as usize);
            }
        }
        for (cx, cy) in p.shape.cells(p.rot) {
            let x = p.x + cx;
            let y = p.y + cy;
            if y >= 0 && (y as usize) < H {
                brick(&mut cv, x as usize, y as usize, p.shape);
            }
        }
    }

    // 锁定白闪：落点格子向白色衰减
    if let Some((cells, t)) = &game.flash {
        let k = (1.0 - t.elapsed().as_secs_f32() / LOCK_FLASH.as_secs_f32()).clamp(0.0, 1.0);
        for &(cx, cy) in cells {
            let (ux, uy) = (cx as usize, cy as usize);
            if uy < H {
                for dy in 0..CELL_H {
                    for dx in 0..CELL_W {
                        fade_px(&mut cv, ux * CELL_W + dx, uy * CELL_H + dy, (255, 255, 255), k);
                    }
                }
            }
        }
    }

    // 消行动画：白光从中心向两侧烧穿
    if let Some(c) = &game.clearing {
        let t = (c.started.elapsed().as_secs_f32() / CLEAR_ANIM.as_secs_f32()).min(1.0);
        let half = (t * (PW / 2) as f32) as usize;
        for &row in &c.rows {
            for y in row as usize * CELL_H..(row as usize + 1) * CELL_H {
                for x in 0..PW {
                    let hole = x >= PW / 2 - half && x < PW / 2 + half;
                    cv.set(x, y, (!hole).then_some(Color::White));
                }
            }
        }
    }

    // 震屏：像素级衰减抖动
    let (mut ox, mut oy) = (inner.x, inner.y);
    if let Some((t, amp)) = &game.shake {
        let k = 1.0 - t.elapsed().as_secs_f32() / SHAKE_TIME.as_secs_f32();
        let a = *amp as f32 * k * 2.0;
        let phase = t.elapsed().as_secs_f32() * 75.0;
        ox = (ox as i32 + (phase.sin() * a) as i32).max(0) as u16;
        oy = (oy as i32 + ((phase * 1.3).cos() * a) as i32).max(0) as u16;
    }
    cv.blit(f.buffer_mut(), ox, oy);

    // 飘分：上浮 + 渐暗
    for p in &game.popups {
        let t = p.born.elapsed().as_secs_f32() / POPUP_TIME.as_secs_f32();
        let color = mix((255, 235, 160), (60, 60, 60), t.min(1.0));
        let cx = inner.x as i32 + (p.x * CELL_W as f32 / 2.0) as i32 - p.text.len() as i32;
        let cy = inner.y as i32 + ((p.y - t * 2.5) * CELL_H as f32 / 2.0) as i32;
        put_text(f.buffer_mut(), cx, cy, &p.text, color);
    }

    if (game.paused || game.over) && inner.height >= 4 && inner.width >= 6 {
        let msg_area = Rect {
            x: inner.x + 1,
            y: inner.y + inner.height / 2 - 1,
            width: inner.width - 2,
            height: 3,
        };
        f.render_widget(Clear, msg_area);
        // 呼吸脉动：亮度随时间正弦起伏（GAME OVER 保持常亮红）
        let glow = 0.18 + 0.18 * (game.born.elapsed().as_secs_f32() * 2.0).sin();
        let (lines, style) = if game.over {
            (
                vec![Line::from("GAME OVER"), Line::from("r 重开 · q 退出")],
                Style::new().fg(Color::Rgb(255, 80, 90)).bold(),
            )
        } else if fresh(game) {
            (
                vec![Line::from("READY"), Line::from("space 开始")],
                Style::new().fg(mix((255, 199, 44), (0, 0, 0), glow)).bold(),
            )
        } else {
            (
                vec![Line::from("PAUSED"), Line::from("space 继续")],
                Style::new().fg(mix((255, 235, 160), (0, 0, 0), glow)).bold(),
            )
        };
        f.render_widget(Paragraph::new(lines).centered().style(style), msg_area);
    }
}

fn draw_panel(f: &mut Frame, game: &Game, area: Rect) {
    if area.width < 8 || area.height < 8 {
        return;
    }
    let chunks = Layout::vertical([Length(4), Length(11), Length(5), Min(0)]).split(area);

    f.render_widget(Block::bordered().title(" HOLD "), chunks[0]);
    if let Some(h) = game.hold {
        draw_mini(
            f,
            h,
            Rect {
                x: chunks[0].x + 1,
                y: chunks[0].y + 1,
                width: chunks[0].width - 2,
                height: chunks[0].height - 2,
            },
        );
    }

    f.render_widget(Block::bordered().title(" NEXT "), chunks[1]);
    let ninner = Rect {
        x: chunks[1].x + 1,
        y: chunks[1].y + 1,
        width: chunks[1].width - 2,
        height: chunks[1].height - 2,
    };
    for (i, s) in game.queue.iter().take(3).enumerate() {
        draw_mini(
            f,
            *s,
            Rect {
                x: ninner.x,
                y: ninner.y + i as u16 * 3,
                width: ninner.width,
                height: 3,
            },
        );
    }

    let stats = Paragraph::new(vec![
        Line::from(format!(" SCORE  {}", game.score)),
        Line::from(format!(" LINES  {}", game.lines)),
        Line::from(format!(" LEVEL  {}", game.level)),
    ])
    .block(Block::bordered().title(" STATS "));
    f.render_widget(stats, chunks[2]);
}

fn draw_mini(f: &mut Frame, shape: Shape, area: Rect) {
    let cells = shape.cells(0);
    let minx = cells.iter().map(|c| c.0).min().unwrap();
    let miny = cells.iter().map(|c| c.1).min().unwrap();
    let maxx = cells.iter().map(|c| c.0).max().unwrap();
    let maxy = cells.iter().map(|c| c.1).max().unwrap();
    let mut cv = Canvas::new(
        (maxx - minx + 1) as usize * CELL_W,
        (maxy - miny + 1) as usize * CELL_H,
    );
    for (cx, cy) in cells {
        brick(&mut cv, (cx - minx) as usize, (cy - miny) as usize, shape);
    }
    let x0 = area.x + area.width.saturating_sub(cv.w as u16 / 2) / 2;
    let y0 = area.y + area.height.saturating_sub(cv.h as u16 / 2) / 2;
    cv.blit(f.buffer_mut(), x0, y0);
}

/// 从未开始：无分且空场（新局 READY 态）。
fn fresh(game: &Game) -> bool {
    game.score == 0 && game.board.iter().all(|row| row.iter().all(|c| c.is_none()))
}

/// 底部按键速查条。
fn draw_help(f: &mut Frame, area: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(
            "h/l move  k/z rot  j drop  J soft  c hold  space pause  r restart  q quit",
        ))
        .centered()
        .style(Style::new().dark_gray()),
        area,
    );
}
