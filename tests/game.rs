use tetris_tui::game::{fits, Game, Shape, W};

/// 回归：锁定后堆叠必须落到底部。曾经的 clear_lines 行序颠倒会把整盘上下翻转，
/// 方块锁在顶行、下一块立刻 GAME OVER，score 永远为 0。
#[test]
fn hard_drop_stacks_at_bottom() {
    let mut g = Game::new();
    g.spawn(Shape::O);
    g.hard_drop();
    assert_eq!(g.board[19][4], Some(Shape::O));
    assert_eq!(g.board[18][5], Some(Shape::O));
    assert_eq!(g.board[0][4], None);
    assert_eq!(g.board[1][5], None);
    assert!(!g.over);
}

#[test]
fn sequential_drops_stack_without_game_over() {
    let mut g = Game::new();
    for _ in 0..4 {
        g.spawn(Shape::I);
        g.hard_drop();
    }
    assert!(!g.over);
    assert_eq!(g.board[19][3], Some(Shape::I));
    assert_eq!(g.board[16][6], Some(Shape::I));
    assert_eq!(g.board[15][3], None);
}

#[test]
fn line_clear_keeps_row_order() {
    let mut g = Game::new();
    for x in 0..W {
        if x != 3 && x != 4 && x != 5 {
            g.board[19][x] = Some(Shape::Z);
        }
    }
    g.board[18][0] = Some(Shape::J);
    g.spawn(Shape::T);
    g.hard_drop(); // T 的 3 格底行补满第 19 行
    assert!(g.is_clearing(), "满行应先进入消行动画");
    g.finish_clear();
    assert_eq!(g.lines, 1);
    // 硬降 18 行 ×2 分 + 单行 100×level1
    assert_eq!(g.score, 36 + 100);
    // 上方行整体下移一格，顺序不颠倒（翻转 bug 会把 J 送到 row1）
    assert_eq!(g.board[19][0], Some(Shape::J));
    assert_eq!(g.board[1][0], None);
    assert_eq!(g.board[0][0], None);
}

#[test]
fn spawn_into_blocked_top_ends_game() {
    let mut g = Game::new();
    for y in 0..2 {
        for x in 3..6 {
            g.board[y][x] = Some(Shape::Z);
        }
    }
    g.spawn(Shape::T);
    assert!(g.over);
}

#[test]
fn hold_swaps_once_per_piece() {
    let mut g = Game::new();
    g.spawn(Shape::I);
    let next = g.queue[0];
    g.hold();
    assert_eq!(g.hold, Some(Shape::I));
    assert_eq!(g.piece.shape, next);
    g.hold(); // 每次锁定前只能 hold 一次
    assert_eq!(g.piece.shape, next);
    g.hard_drop();
    g.hold(); // 锁定后恢复
    assert_eq!(g.piece.shape, Shape::I);
}

#[test]
fn gravity_locks_when_landing() {
    let mut g = Game::new();
    g.spawn(Shape::O);
    while !g.over && g.board[18][4].is_none() {
        g.tick();
    }
    assert_eq!(g.board[19][4], Some(Shape::O));
    assert_eq!(g.board[18][4], Some(Shape::O));
}

/// 不变量：任意按键序列后，当前方块必须处于合法（无重叠、在场内或顶部悬空）位置。
#[test]
fn current_piece_always_in_legal_state() {
    let mut g = Game::new();
    let keys: [fn(&mut Game); 6] = [
        |g| {
            g.try_move(-1, 0);
        },
        |g| {
            g.try_move(1, 0);
        },
        |g| g.rotate(1),
        |g| g.rotate(3),
        |g| {
            g.tick();
        },
        |g| g.hard_drop(),
    ];
    for i in 0..300 {
        if g.over {
            g = Game::new();
        }
        keys[i % keys.len()](&mut g);
        if g.is_clearing() {
            g.finish_clear(); // 模拟主循环：动画结束即收拢，保持有活动方块
        }
        let p = g.piece;
        // over 时允许 spawn 位被占（那是结束的原因）；进行中局面方块必须合法。
        if !g.over {
            assert!(fits(&g.board, p.shape, p.rot, p.x, p.y), "step {i}: {p:?}");
        }
    }
}

/// 消行动画期间没有活动方块：位移/变形/硬降/重力全部挂起。
#[test]
fn clearing_freezes_all_piece_ops() {
    let mut g = Game::new();
    for x in 0..W {
        if x != 4 && x != 5 {
            g.board[19][x] = Some(Shape::Z);
        }
    }
    g.spawn(Shape::O);
    g.hard_drop(); // O 补满第 19 行 → 进入动画
    assert!(g.is_clearing());
    let p = (g.piece.x, g.piece.rot, g.piece.y);
    assert!(!g.try_move(1, 0));
    g.rotate(1);
    g.hard_drop();
    g.tick();
    assert_eq!((g.piece.x, g.piece.rot, g.piece.y), p);
    assert!(g.board[19][4].is_some(), "动画期间行未收拢");
    g.finish_clear();
    assert!(!g.is_clearing());
    assert_eq!(g.lines, 1);
    assert_eq!(g.board[19][4], Some(Shape::O), "上方未消内容整体下移一格");
}
