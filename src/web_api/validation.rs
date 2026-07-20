const ITEM_ID_RE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
const MAX_ITEM_ID_LEN: usize = 64;
const MAX_SERVER_NAME_LEN: usize = 64;

pub fn valid_item_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ITEM_ID_LEN && id.bytes().all(|b| ITEM_ID_RE.contains(&b))
}

pub fn valid_server_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SERVER_NAME_LEN
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}
