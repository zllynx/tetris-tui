use std::collections::VecDeque;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;

pub const W: usize = 10;
pub const H: usize = 20;

pub type Board = [[Option<Shape>; W]; H];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

pub const ALL: [Shape; 7] = [
    Shape::I,
    Shape::O,
    Shape::T,
    Shape::S,
    Shape::Z,
    Shape::J,
    Shape::L,
];
const LINE_SCORE: [u32; 5] = [0, 100, 300, 500, 800];

/// 消行动画时长：满行闪烁这段时间后收拢结算。
pub const CLEAR_ANIM: Duration = Duration::from_millis(200);
/// 锁定白闪时长。
pub const LOCK_FLASH: Duration = Duration::from_millis(130);
/// 硬降拖尾时长。
pub const TRAIL_TIME: Duration = Duration::from_millis(170);
/// 震屏时长。
pub const SHAKE_TIME: Duration = Duration::from_millis(140);
/// 飘分存活时间。
pub const POPUP_TIME: Duration = Duration::from_millis(700);

impl Shape {
    /// 旋转所在方阵边长；I 在 4x4，O 在 2x2（旋转不变），其余 3x3。
    fn size(self) -> i8 {
        match self {
            Shape::I => 4,
            Shape::O => 2,
            _ => 3,
        }
    }

    /// 返回该旋转态的 4 个格子坐标 (x, y)，y 向下为正。
    pub fn cells(self, rot: u8) -> [(i8, i8); 4] {
        let base: [(i8, i8); 4] = match self {
            Shape::I => [(0, 1), (1, 1), (2, 1), (3, 1)],
            Shape::O => [(0, 0), (1, 0), (0, 1), (1, 1)],
            Shape::T => [(1, 0), (0, 1), (1, 1), (2, 1)],
            Shape::S => [(1, 0), (2, 0), (0, 1), (1, 1)],
            Shape::Z => [(0, 0), (1, 0), (1, 1), (2, 1)],
            Shape::J => [(0, 0), (0, 1), (1, 1), (2, 1)],
            Shape::L => [(2, 0), (0, 1), (1, 1), (2, 1)],
        };
        let n = self.size();
        let mut c = base;
        for _ in 0..(rot % 4) {
            c = c.map(|(x, y)| (n - 1 - y, x));
        }
        c
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub rot: u8,
    pub x: i8,
    pub y: i8,
}

/// 进行中的消行动画：满行行号 + 起始时刻。
pub struct Clearing {
    pub rows: Vec<u8>,
    pub started: Instant,
}

/// 硬降拖尾：每条为 (格子列 x, 起始行, 落底行)，格坐标。
pub struct Trail {
    pub cols: Vec<(i8, i16, i16)>,
    pub shape: Shape,
    pub born: Instant,
}

/// 飘分文字：格坐标（f32 便于亚格上浮）。
pub struct Popup {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub born: Instant,
}

pub struct Game {
    pub board: Board,
    pub piece: Piece,
    pub queue: VecDeque<Shape>,
    pub hold: Option<Shape>,
    pub score: u32,
    pub lines: u32,
    pub level: u32,
    /// 新局为 true（READY 态），空格开始/暂停。
    pub paused: bool,
    pub over: bool,
    /// 消行动画进行时；此间 piece 为已锁定的旧值，一切位移被忽略。
    pub clearing: Option<Clearing>,
    /// 新局时刻，用于呼吸动画相位。
    pub born: Instant,
    /// 锁定白闪：落点格子坐标 + 起始时刻。
    pub flash: Option<(Vec<(u8, u8)>, Instant)>,
    /// 硬降拖尾。
    pub trail: Option<Trail>,
    /// 震屏：起始时刻 + 强度档。
    pub shake: Option<(Instant, u8)>,
    /// 飘分文字。
    pub popups: Vec<Popup>,
    bag: Vec<Shape>,
    can_hold: bool,
}

/// 该姿态的所有格子是否都落在场内且不与已锁定的块重叠（场上方的 y<0 视为空）。
pub fn fits(b: &Board, shape: Shape, rot: u8, px: i8, py: i8) -> bool {
    shape.cells(rot).iter().all(|&(cx, cy)| {
        let x = px + cx;
        let y = py + cy;
        if x < 0 || x >= W as i8 || y >= H as i8 {
            return false;
        }
        y < 0 || b[y as usize][x as usize].is_none()
    })
}

impl Game {
    pub fn new() -> Self {
        let mut g = Game {
            board: [[None; W]; H],
            piece: Piece {
                shape: Shape::I,
                rot: 0,
                x: 3,
                y: 0,
            },
            queue: VecDeque::new(),
            hold: None,
            score: 0,
            lines: 0,
            level: 1,
            paused: true,
            over: false,
            clearing: None,
            born: Instant::now(),
            flash: None,
            trail: None,
            shake: None,
            popups: Vec::new(),
            bag: Vec::new(),
            can_hold: true,
        };
        g.refill();
        let first = g.queue.pop_front().unwrap();
        g.spawn(first);
        g
    }

    /// 是否正在播放消行动画（此间没有活动方块）。
    pub fn is_clearing(&self) -> bool {
        self.clearing.is_some()
    }

    /// 消行动画是否已播完，主循环据此调用 finish_clear。
    pub fn clear_anim_done(&self) -> bool {
        self.clearing
            .as_ref()
            .is_some_and(|c| c.started.elapsed() >= CLEAR_ANIM)
    }

    /// 推进特效状态：过期即清除。主循环每帧调用。
    pub fn tick_effects(&mut self) {
        let now = Instant::now();
        if self.flash.as_ref().is_some_and(|(_, t)| now.duration_since(*t) >= LOCK_FLASH) {
            self.flash = None;
        }
        if self
            .trail
            .as_ref()
            .is_some_and(|t| now.duration_since(t.born) >= TRAIL_TIME)
        {
            self.trail = None;
        }
        if self.shake.is_some_and(|(t, _)| now.duration_since(t) >= SHAKE_TIME) {
            self.shake = None;
        }
        self.popups.retain(|p| now.duration_since(p.born) < POPUP_TIME);
    }

    fn add_popup(&mut self, text: String, x: f32, y: f32) {
        let now = Instant::now();
        self.popups.retain(|p| now.duration_since(p.born) < POPUP_TIME);
        if self.popups.len() >= 6 {
            self.popups.remove(0);
        }
        self.popups.push(Popup {
            text,
            x,
            y,
            born: now,
        });
    }

    /// 7-bag 随机器，维持 next 队列至少 5 个。
    fn refill(&mut self) {
        while self.queue.len() < 5 {
            if self.bag.is_empty() {
                let mut b: Vec<Shape> = ALL.to_vec();
                b.shuffle(&mut thread_rng());
                self.bag = b;
            }
            self.queue.push_back(self.bag.pop().unwrap());
        }
    }

    pub fn spawn(&mut self, shape: Shape) {
        let x = if shape == Shape::O { 4 } else { 3 };
        self.piece = Piece {
            shape,
            rot: 0,
            x,
            y: 0,
        };
        if !fits(&self.board, shape, 0, x, 0) {
            self.over = true;
        }
    }

    pub fn try_move(&mut self, dx: i8, dy: i8) -> bool {
        if self.is_clearing() {
            return false;
        }
        let p = self.piece;
        if fits(&self.board, p.shape, p.rot, p.x + dx, p.y + dy) {
            self.piece.x += dx;
            self.piece.y += dy;
            true
        } else {
            false
        }
    }

    /// 带简单踢墙的旋转：依次尝试水平偏移和上移一格。
    pub fn rotate(&mut self, dr: u8) {
        if self.is_clearing() {
            return;
        }
        let p = self.piece;
        let nr = (p.rot + dr) % 4;
        for (dx, dy) in [(0i8, 0i8), (-1, 0), (1, 0), (-2, 0), (2, 0), (0, -1)] {
            if fits(&self.board, p.shape, nr, p.x + dx, p.y + dy) {
                self.piece.rot = nr;
                self.piece.x += dx;
                self.piece.y += dy;
                return;
            }
        }
    }

    pub fn ghost_y(&self) -> i8 {
        let p = self.piece;
        let mut y = p.y;
        while fits(&self.board, p.shape, p.rot, p.x, y + 1) {
            y += 1;
        }
        y
    }

    pub fn hard_drop(&mut self) {
        if self.is_clearing() {
            return;
        }
        let gy = self.ghost_y();
        let dist = (gy - self.piece.y).max(0);
        self.score += 2 * dist as u32;
        let p = self.piece;
        let now = Instant::now();
        if dist >= 2 {
            // 拖影：每个占用格列记一条从起点到落点的竖条
            let cols = p
                .shape
                .cells(p.rot)
                .iter()
                .map(|&(cx, cy)| (p.x + cx, (p.y + cy).max(0) as i16, (gy + cy) as i16))
                .collect();
            self.trail = Some(Trail {
                cols,
                shape: p.shape,
                born: now,
            });
        }
        if dist >= 4 {
            self.shake = Some((now, 1));
        }
        let cells = p.shape.cells(p.rot);
        let minx = cells.iter().map(|c| c.0).min().unwrap();
        let maxx = cells.iter().map(|c| c.0).max().unwrap();
        if dist > 0 {
            let cx = p.x as f32 + (minx + maxx + 1) as f32 / 2.0;
            self.add_popup(format!("+{}", 2 * dist), cx, gy as f32);
        }
        self.piece.y = gy;
        self.lock();
    }

    pub fn tick(&mut self) {
        if self.is_clearing() {
            return;
        }
        if !self.try_move(0, 1) {
            self.lock();
        }
    }

    fn lock(&mut self) {
        let p = self.piece;
        for (cx, cy) in p.shape.cells(p.rot) {
            let x = p.x + cx;
            let y = p.y + cy;
            if y >= 0 && y < H as i8 {
                self.board[y as usize][x as usize].replace(p.shape);
            }
        }
        let cells: Vec<(u8, u8)> = p
            .shape
            .cells(p.rot)
            .iter()
            .filter_map(|&(cx, cy)| {
                let (x, y) = (p.x + cx, p.y + cy);
                (y >= 0).then_some((x as u8, y as u8))
            })
            .collect();
        self.flash = Some((cells, Instant::now()));
        let full: Vec<u8> = (0..H as u8)
            .filter(|&y| self.board[y as usize].iter().all(|c| c.is_some()))
            .collect();
        if full.is_empty() {
            self.finish_clear(); // 无消行：直接结算并生成下一块
        } else {
            self.clearing = Some(Clearing {
                rows: full,
                started: Instant::now(),
            });
        }
    }

    /// 消行动画结束（或无消行锁定）时：收拢满行、结算分数、生成下一块。
    pub fn finish_clear(&mut self) {
        let clearing = self.clearing.take();
        let cleared = match &clearing {
            Some(c) => c.rows.len() as u32,
            None => 0,
        };
        self.flash = None; // 落点已在消行里，白闪无意义
        if cleared > 0 {
            let rows = &clearing.as_ref().unwrap().rows;
            let y_top = *rows.iter().min().unwrap();
            self.add_popup(
                format!("+{}", LINE_SCORE[cleared as usize] * self.level),
                W as f32 / 2.0,
                y_top as f32,
            );
            if cleared >= 2 {
                self.shake = Some((Instant::now(), cleared as u8));
            }
            self.clear_lines();
        }
        self.score += LINE_SCORE[cleared as usize] * self.level;
        self.lines += cleared;
        self.level = self.lines / 10 + 1;
        self.can_hold = true;
        self.refill();
        let next = self.queue.pop_front().unwrap();
        self.spawn(next);
    }

    /// 自底向上收拢：满行删除，上方整块下移，行序不变。
    fn clear_lines(&mut self) -> u32 {
        let mut next: Board = [[None; W]; H];
        let mut write = H;
        let mut cleared = 0;
        for row in (0..H).rev() {
            if self.board[row].iter().all(|c| c.is_some()) {
                cleared += 1;
            } else {
                write -= 1;
                next[write] = self.board[row];
            }
        }
        self.board = next;
        cleared
    }

    pub fn hold(&mut self) {
        if !self.can_hold || self.over || self.is_clearing() {
            return;
        }
        self.can_hold = false;
        let cur = self.piece.shape;
        let next = match self.hold {
            Some(h) => {
                self.hold = Some(cur);
                h
            }
            None => {
                self.hold = Some(cur);
                self.refill();
                self.queue.pop_front().unwrap()
            }
        };
        self.spawn(next);
    }

    pub fn drop_interval(&self) -> Duration {
        let ms = (500.0 * 0.8f64.powi((self.level - 1) as i32)).max(60.0) as u64;
        Duration::from_millis(ms)
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
