use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(alias = "messageType", alias = "MessageType")]
    pub message_type: String,
    #[serde(alias = "data", alias = "Data")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDataChangedInfo {
    #[serde(alias = "userId", alias = "UserId")]
    pub user_id: String,
    #[serde(alias = "userDataList", alias = "UserDataList")]
    pub user_data_list: Vec<UserDataEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDataEntry {
    #[serde(alias = "itemId", alias = "ItemId")]
    pub item_id: String,
    #[serde(alias = "played", alias = "Played")]
    pub played: bool,
    #[serde(alias = "playbackPositionTicks", alias = "PlaybackPositionTicks")]
    pub playback_position_ticks: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    #[serde(alias = "id", alias = "Id")]
    pub id: String,
    #[serde(alias = "userName", alias = "UserName")]
    pub user_name: Option<String>,
    #[serde(alias = "nowPlayingItem", alias = "NowPlayingItem")]
    pub now_playing_item: Option<NowPlayingItem>,
    #[serde(alias = "playState", alias = "PlayState")]
    pub play_state: Option<PlayState>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NowPlayingItem {
    #[serde(alias = "id", alias = "Id")]
    pub id: String,
    #[serde(alias = "name", alias = "Name")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlayState {
    #[serde(alias = "positionTicks", alias = "PositionTicks")]
    pub position_ticks: Option<i64>,
    #[serde(alias = "isPaused", alias = "IsPaused")]
    pub is_paused: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PlayedItem {
    #[serde(alias = "Id", alias = "id")]
    pub id: String,
    #[serde(default, alias = "Played", alias = "played")]
    pub played: bool,
    #[serde(
        default,
        alias = "PlaybackPositionTicks",
        alias = "playbackPositionTicks"
    )]
    pub playback_position_ticks: Option<i64>,
    #[serde(default, alias = "LastPlayedDate", alias = "lastPlayedDate")]
    pub last_played_date: Option<String>,
    #[serde(default)]
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub tmdb_id: Option<String>,
}
