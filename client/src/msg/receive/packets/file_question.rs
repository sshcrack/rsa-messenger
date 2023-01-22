use inquire::Confirm;
use colored::Colorize;
use tokio_tungstenite::tungstenite::Message;

use crate::util::{tools::{uuid_from_vec, uuid_to_name}, modes::Modes, msg::{send_msg, lock_input, release_input}};

pub async fn on_file_question(
    data: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let sender = uuid_from_vec(data)?;
    let filename = String::from_utf8(data.to_owned())?;

    let sender_name = uuid_to_name(sender).await?;

    println!("Locking input...");
    lock_input().await?;

    let confirm_msg = format!("{} wants to send {}. Accept? (y/n)", sender_name.green(), filename.yellow());
    let accepted = Confirm::new(&confirm_msg)
    .prompt();

    println!("Releasing input...");
    release_input().await?;

    let accepted = accepted?;
    if !accepted {
        let denied = format!("You denied {} send request.", filename.bright_red());
        println!("{}", denied.red())
    } else {
        let allowed = format!("Receiving {}{}", filename.bright_green(), "...".yellow());
        println!("{}", allowed.green());
    }

    let accepted_vec: u8 = if accepted == true { 0} else { 1 };
    let to_send = Modes::SendFileQuestionReply.get_send(&vec![accepted_vec]);

    send_msg(Message::binary(to_send)).await?;
    Ok(())
}
