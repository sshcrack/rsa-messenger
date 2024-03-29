#[derive(Copy, Clone)]
pub enum Modes {
    SetPubkey,
    To,
    From,
    Name,
    WantUid,
    UidReply,
    Error,
    SymmKey,
    WantSymmKey,
    // Initial Request from client to other client for asking them wether they want to receive the file or not
    SendFileQuestion,
    // Self-explanatory
    SendFileQuestionReply,
    // Message when other chunk of data is ready to be downloaded
    SendFileChunkReady,
    // Message when the receiver has downloaded the chunk and is ready for the next one
    SendFileChunkDownloaded,
    // Sent from server to sending client, to retrieve file size etc
    SendFileStartProcessing,
    SendFileAbort
}

impl Modes {
    pub fn get_indicator(self) -> u8 {
        match self {
            Self::SetPubkey => 0,
            Self::To => 1,
            Self::From => 2,
            Self::Name => 3,
            Self::WantUid => 4,
            Self::UidReply => 5,
            Self::Error => 6,
            Self::SendFileQuestion => 7,
            Self::SendFileQuestionReply => 8,
            Self::SendFileChunkReady => 9,
            Self::SendFileChunkDownloaded => 10,
            Self::SendFileStartProcessing => 11,
            Self::SendFileAbort => 12,
            Self::SymmKey => 13,
            Self::WantSymmKey => 14
        }
    }

    pub fn is_indicator(self, b: &u8) -> bool {
        let ind = self.get_indicator();
        return ind.eq(b);
    }

    pub fn get_send(self, end: &[u8]) -> Vec<u8> {
        let ind = self.get_indicator();

        let mut el = end.to_vec().clone();
        el.reverse();

        el.push(ind);

        el.reverse();

        return el;
    }
}
