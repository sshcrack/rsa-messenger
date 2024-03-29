use std::collections::VecDeque;

use anyhow::anyhow;
use colored::Colorize;
use futures_util::StreamExt;
use packets::util::modes::Modes;
use packets::util::vec::decque_to_vec;
use tokio_tungstenite::tungstenite::Message;

use crate::util::types::*;

use super::packets::error::on_error;
use super::packets::file::chunk::abort::on_chunk_abort;
use super::packets::file::chunk::downloaded::on_chunk_downloaded;
use super::packets::file::chunk::ready::on_chunk_ready;
use super::packets::file::question::index::on_file_question;
use super::packets::file::question::reply::on_file_question_reply;
use super::packets::file::start_processing::on_start_processing;
use super::packets::symm_key::on_symm_key;
use super::packets::want_symm_key::on_want_symm_key;
use super::packets::{from::on_from, uid::on_uid};

pub async fn receive_msgs(mut rx: RXChannel) -> anyhow::Result<()> {
    while let Some(msg) = rx.next().await {
        let msg = msg?;
        tokio::spawn(async move {
            let res = handle(msg).await;
            if res.is_err() {
                let err = res.unwrap_err();
                if err.to_string().contains("Operation was interrupted by the user") {
                    std::process::exit(0);
                }

                eprintln!("Error occurred while processing message packet: ");
                eprintln!(
                    "{}",
                    format!("{:?}", err).on_bright_red().black()
                );
            }
        });
    }

    Ok(())
}

pub async fn handle(msg: Message) -> anyhow::Result<()> {
    let data = msg.into_data();
    let mut decque: VecDeque<u8> = VecDeque::new();
    for i in data {
        decque.push_back(i);
    }

    let mode = decque.pop_front();
    if mode.is_none() {
        return Ok(());
    }

    let mode = mode.unwrap();
    let mut data = decque_to_vec(decque);

    if Modes::From.is_indicator(&mode) {
        on_from(&mut data).await?;
        return Ok(());
    }

    if Modes::UidReply.is_indicator(&mode) {
        on_uid(&mut data).await?;
        return Ok(());
    }

    if Modes::SendFileQuestion.is_indicator(&mode) {
        on_file_question(&mut data).await?;
        return Ok(());
    }

    if Modes::SendFileQuestionReply.is_indicator(&mode) {
        on_file_question_reply(&mut data).await?;
        return Ok(());
    }

    if Modes::Error.is_indicator(&mode) {
        on_error(&mut data).await?;
        return Ok(());
    }

    if Modes::SendFileStartProcessing.is_indicator(&mode) {
        on_start_processing(&mut data).await?;
        return Ok(());
    }

    if Modes::SendFileChunkReady.is_indicator(&mode) {
        on_chunk_ready(&mut data).await?;
        return Ok(());
    }

    if Modes::SendFileChunkDownloaded.is_indicator(&mode) {
        on_chunk_downloaded(&mut data).await?;
        return Ok(());
    }

    if Modes::SendFileAbort.is_indicator(&mode) {
        on_chunk_abort(&mut data).await?;
        return Ok(())
    }

    if Modes::SymmKey.is_indicator(&mode) {
        on_symm_key(&mut data).await?;
        return Ok(());
    }

    if Modes::WantSymmKey.is_indicator(&mode) {
        on_want_symm_key(&mut data).await?;
        return Ok(());
    }

    return Err(anyhow!("Invalid packet received."));
}
