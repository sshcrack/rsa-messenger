use packets::{initialize::uid_reply::UidReplyMsg, types::WSMessage};
use uuid::Uuid;
use warp::ws::Message;

use crate::utils::{types::TXChannel, tools::send_msg};

pub fn on_uid(my_id: &Uuid, tx: &TXChannel) -> anyhow::Result<()> {
    let to_send = UidReplyMsg {
        id: my_id.clone()
    }.serialize();

    send_msg(tx, Message::binary(to_send))?;
    Ok(())
}
