use anyhow::anyhow;
use packets::other::info::UserInfoBasic;
use uuid::Uuid;

use crate::file::consts::USERS;

pub async fn get_user(uuid: &Uuid) -> anyhow::Result<UserInfoBasic> {
    let state = USERS.read().await;

    let info = state.get(uuid);
    if info.is_none() {
        drop(state);
        return Err(anyhow!("No user info for uuid found."));
    }

    let info = info.unwrap().to_basic();
    drop(state);

    return Ok(info);
}
