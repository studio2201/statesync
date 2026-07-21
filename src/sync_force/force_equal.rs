use crate::client::types::UserDataEntry;

/// True if force should skip write because target already matches source played/position.
pub fn played_state_already_equal(
    force_played: bool,
    force_position: bool,
    source_pos: i64,
    tgt: &UserDataEntry,
) -> bool {
    const POS_EQ_TICKS: u64 = 50_000_000; // 5 seconds
    let mut need_write = false;
    if force_played && !tgt.played {
        need_write = true;
    }
    if force_position {
        let tgt_pos = tgt.playback_position_ticks.unwrap_or(0);
        if source_pos.abs_diff(tgt_pos) > POS_EQ_TICKS {
            if source_pos > 0 || tgt_pos > 0 {
                need_write = true;
            }
        }
    }
    !need_write
}
