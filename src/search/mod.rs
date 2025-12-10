use crate::Board;

pub struct SearchEngine {
    // TODO: make these structs
    // transposition_table: TranspositionTable,
    // killer_moves: KillerMoveTable,
    // history_table: HistoryTable,
    nodes_searched: u64,
    // search_stats: SearchStats,
}

fn negamax(board: &Board, depth: i32, mut alpha: i32, beta: i32, ply: u8) -> i32 {
    unimplemented!()
}
