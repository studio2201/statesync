/// Human live-sync message for progress/played events.
pub fn format_progress_message(
    user_name: &str,
    item_title: &str,
    position: i64,
    played: bool,
    send_played: bool,
) -> String {
    if played && send_played {
        return format!("{} finished watching '{}'", user_name, item_title);
    }
    let pos_secs = position as f64 / 10_000_000.0;
    let h = (pos_secs / 3600.0).floor() as u32;
    let m = ((pos_secs % 3600.0) / 60.0).floor() as u32;
    let s = (pos_secs % 60.0).floor() as u32;
    let duration_str = if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    };
    format!(
        "{} synced progress on '{}' to {}",
        user_name, item_title, duration_str
    )
}
