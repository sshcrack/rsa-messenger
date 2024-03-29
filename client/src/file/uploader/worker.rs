use std::{io::SeekFrom, path::Path, sync::Arc};

use anyhow::anyhow;
use bytes::{BytesMut, Buf};
use log::{debug, trace, warn};
use openssl::{pkey::Public, rsa::Rsa};
use packets::{
    consts::{CHUNK_SIZE, ONE_MB_SIZE},
    file::{processing::tools::get_max_chunks, types::FileInfo, chunk::index::ChunkMsg}, encryption::sign::get_signature, other::key_iv::KeyIVPair
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, BufReader},
    sync::{RwLock, mpsc::UnboundedSender},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{util::arcs::{get_curr_keypair, get_base_url}, web::{prefix::get_web_protocol, progress::upload_file}, file::tools::WorkerProgress};

pub type ProgressTX = UnboundedSender<WorkerProgress>;
pub type ArcProgressTX = Arc<RwLock<ProgressTX>>;

#[derive(Debug)]
pub struct UploadWorker {
    worker_id: u64,
    uuid: Uuid,
    file: FileInfo,
    thread: Option<JoinHandle<anyhow::Result<()>>>,
    running: bool,
    tx: ArcProgressTX,
    receiver_key: Rsa<Public>,
    pub curr_chunk: Arc<RwLock<Option<u64>>>,
    aborted: Arc<RwLock<bool>>
}

impl UploadWorker {
    pub fn new(
        worker_id: u64,
        uuid: Uuid,
        receiver_key: Rsa<Public>,
        file: FileInfo,
        progress_channel: ArcProgressTX,
    ) -> anyhow::Result<UploadWorker> {
        let FileInfo { filename, size,path, .. } = file.clone();
        if path.is_none() {
            return Err(anyhow!("Can not upload file when path is not given."));
        }

        let path = path.unwrap();
        let path = Path::new(&path);

        if !path.is_file() {
            eprintln!("Could not send file at {} (does not exist)", filename);
            return Err(anyhow!("File '{}' does not exist.", filename));
        }

        let metadata = path.metadata()?;
        if metadata.len() != size {
            eprintln!(
                "Size of file does not match with metadata (metadata {}, given {})",
                metadata.len(),
                file.size
            );
            return Err(anyhow!("Size of file does not match with metadata"));
        }

        return Ok(UploadWorker {
            worker_id,
            file,
            uuid,
            thread: None,
            tx: progress_channel,
            running: false,
            receiver_key,
            curr_chunk: Arc::new(RwLock::new(None)),
            aborted: Arc::new(RwLock::new(false))
        });
    }

    async fn spawn_thread(&self, chunk_index: u64) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
        if *self.aborted.read().await {
            return Err(anyhow!("Cannot start worker as it has been aborted."))
        }

        trace!(
            "Spawning new worker with i: {} uuid: {}",
            chunk_index,
            self.uuid
        );

        let file = self.file.clone();
        let tx = self.tx.clone();
        let curr_chunk_arc = self.curr_chunk.clone();
        let uuid = self.uuid.clone();

        let i = chunk_index;

        let path = file.path.unwrap();
        let size = file.size;
        let receiver_key = self.receiver_key.clone();

        let mut state = self.curr_chunk.write().await;
        state.replace(chunk_index);

        drop(state);

        let handle: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            let to_run = || async {
                let max_chunks = get_max_chunks(size);

                if max_chunks <= 0 {
                    warn!("Max Threads is 0 in index {}", i);
                    return Err(anyhow!("MaxThreads is zero"));
                }

                trace!(
                    "While loop with i: {} and max_threads {}...",
                    i,
                    max_chunks
                );

                let f = File::open(&path).await?;
                let mut buf = BufReader::new(f);
                let seek_to = i64::try_from(CHUNK_SIZE * i)?;

                if (seek_to as u64) > size {
                    trace!("Invalid upload error with index {}", i);
                    return Err(anyhow!(format!("Can not seek to {} as file size is only {}", seek_to, size)));
                }

                trace!("Seeek {}", seek_to);
                buf.seek(SeekFrom::Current(seek_to)).await?;

                let is_last_chunk = (i + max_chunks) >= max_chunks;

                let chunk_size_u64 = if is_last_chunk {
                    std::cmp::min(CHUNK_SIZE, size)
                } else {
                    CHUNK_SIZE
                };
                let chunk_size = usize::try_from(chunk_size_u64)?;

                let mut chunk = Vec::with_capacity(chunk_size);

                let mut bytes_read = 0;
                trace!("while loop {}", seek_to);
                while bytes_read < chunk_size {
                    let to_read = std::cmp::min(ONE_MB_SIZE, chunk_size_u64);
                    let to_read = usize::try_from(to_read)?;
                    let mut small_chunk = BytesMut::with_capacity(to_read);

                    buf.read_buf(&mut small_chunk).await?;
                    chunk.append(&mut small_chunk.chunk().to_vec());

                    let progress = (bytes_read as f32) / (chunk_size as f32) * 0.5;
                    let tx = tx.read().await;
                    let e = tx.send(WorkerProgress {
                        progress,
                        chunk: i
                    });
                    drop(tx);
                    e?;

                    bytes_read += to_read;
                }

                let key = KeyIVPair::generate()?;
                let encrypted = key.encrypt(&chunk.to_vec())?;

                let keypair = get_curr_keypair().await?;
                let signature = get_signature(&encrypted.clone(), &keypair)?;

                let base_url = get_base_url().await;
                let http_protocol = get_web_protocol().await;

                let url = format!("{}//{}/file/upload", http_protocol, base_url);

                let body = ChunkMsg {
                    signature,
                    encrypted,
                    uuid,
                    chunk_index: i,
                    key
                }.serialize(&receiver_key)?;
                trace!("Uploading chunk {} to {} with size {}...", i, url, body.len());

                let res = upload_file(url, body,  tx.clone(), 0.5, chunk_index).await;
                let mut res = res?;

                let status = res.status();
                let e = res.body_string().await;
                if status != 200 {
                    eprintln!("Error uploading file: {}", e.unwrap_or("unknown err".to_string()));
                }

                let tx = tx.read().await;
                let e = tx.send(WorkerProgress {
                    progress: 1 as f32,
                    chunk: i
                });
                drop(tx);
                e?;
                debug!("Worker {} of file {} done.", i, uuid);
                Ok(())
            };

            let res = to_run().await;

            let mut state = curr_chunk_arc.write().await;
            state.take();

            drop(state);

            res?;
            Ok(())
        });

        return Ok(handle);
    }

    pub async fn start(&mut self, chunk_index: u64) -> anyhow::Result<()> {
        if self.thread.is_some() {
            trace!(
                "Could not start thread on index {}. Already running.",
                chunk_index
            );
            return Err(anyhow!(format!(
                "Could not start new thread. Already running. Index: {}",
                chunk_index
            )));
        }

        self.running = true;
        let thread = self.spawn_thread(chunk_index).await?;
        self.thread = Some(thread);

        Ok(())
    }

    pub async fn wait_for_end(&mut self) -> anyhow::Result<()> {
        if self.thread.is_none() {
            warn!("Thread is none. Could not wait for end so returning instantly.");
            return Ok(());
        }

        let res = self.thread.take().unwrap();

        let e = res.await;
        if e.is_err() {
            trace!("Checking for join err:");
        }
        let e = e?;
        if e.is_err() {
            trace!("Checking for runtime error:");
        }
        e?;

        trace!("Worker end.");
        self.running = false;

        return Ok(());
    }

    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        return self.running;
    }

    pub fn get_working_id(&self) -> u64 {
        return self.worker_id;
    }
}
