use async_trait::async_trait;
use chrono::{Duration, TimeZone, Utc};
use chrono_tz::US::Pacific;
use config::Config;
use env_logger::Env;
use rust_tdlib::client::tdlib_client::TdJson;
use rust_tdlib::types::{
    AuthorizationStateWaitCode, AuthorizationStateWaitEncryptionKey,
    AuthorizationStateWaitPassword, AuthorizationStateWaitPhoneNumber,
    AuthorizationStateWaitRegistration,
};
use rust_tdlib::types::{DownloadFile, GetChat, GetChatHistory, GetChats, MessageContent};
use rust_tdlib::{
    client::AuthStateHandler,
    client::{Client, ClientState, Worker},
    tdjson,
    types::{GetMe, TdlibParameters, Update},
};
use slug::slugify;
use std::borrow::Borrow;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// ****************************
/// Begin Auth Handler
/// ****************************

/// Define an AuthStateHandler that gives an empty password for the database file, but asks for user input for everything else
///
/// Got the idea from https://github.com/antonio-antuan/rust-tdlib/issues/11
#[derive(Debug, Clone)]
pub struct ConsoleAndEmptyAuthStateHandler;

impl ConsoleAndEmptyAuthStateHandler {
    pub fn new() -> Self {
        Self
    }

    fn wait_input() -> String {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => input.trim().to_string(),
            Err(e) => panic!("Can not get input value: {:?}", e),
        }
    }
}

impl Default for ConsoleAndEmptyAuthStateHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthStateHandler for ConsoleAndEmptyAuthStateHandler {
    async fn handle_wait_code(&self, _wait_code: &AuthorizationStateWaitCode) -> String {
        println!("Please enter the auth code that you just got on Telegram.");
        ConsoleAndEmptyAuthStateHandler::wait_input()
    }

    async fn handle_encryption_key(
        &self,
        _wait_encryption_key: &AuthorizationStateWaitEncryptionKey,
    ) -> String {
        log::info!(
            "being asked for an encryption password/key for the database; using empty string"
        );
        "".to_owned()
    }

    async fn handle_wait_password(
        &self,
        _wait_password: &AuthorizationStateWaitPassword,
    ) -> String {
        panic!("Auth flow weirdness: why is it asking for a password?  Please don't use this program to *sign up* for Telegram.");
    }

    async fn handle_wait_phone_number(
        &self,
        _wait_phone_number: &AuthorizationStateWaitPhoneNumber,
    ) -> String {
        println!("Please enter your telegram phone number (including +NNN, where NNN is your country code); this should only happen on the first run");
        ConsoleAndEmptyAuthStateHandler::wait_input()
    }

    async fn handle_wait_registration(
        &self,
        _wait_registration: &AuthorizationStateWaitRegistration,
    ) -> (String, String) {
        panic!("Auth flow weirdness: why is it asking for first and last name?  Please don't use this program to *sign up* for Telegram.");
    }
}

/// ****************************
/// End Auth Handler
/// ****************************

async fn my_download_file(
    client: &Client<TdJson>,
    receiver: &mut tokio::sync::mpsc::Receiver<Box<Update>>,
    date_time: chrono::DateTime<chrono_tz::Tz>,
    chat_title: &str,
    file_id: i32,
    file_type: &str,
) {
    client
        .download_file(
            DownloadFile::builder()
                .file_id(file_id)
                .synchronous(true)
                .priority(1),
        )
        .await
        .unwrap();

    // Waiting for an update like this:
    //
    // [2023-01-05T08:10:52Z INFO  mytest] file message received: UpdateFile { extra: None, client_id: Some(1), file: File { extra: None, client_id: None, id: 1385, size: 57569, expected_size: 57569, local: LocalFile { extra: None, client_id: None, path: "/tmp/test/mytest/tddb1/photos/5596157149499729848_121.jpg", can_be_downloaded: true, can_be_deleted: true, is_downloading_active: false, is_downloading_completed: true, download_offset: 0, downloaded_prefix_size: 57569, downloaded_size: 57569 }, remote: RemoteFile { extra: None, client_id: None, id: "AgACAgEAAx0EXJpHswADq2O2hgvPOXjH1K1ImIeyKCdVSjvkAAK4pzEbR4upTUatfvTUyNV4AQADAgADeQADIwQ", unique_id: "AQADuKcxG0eLqU1-", is_uploading_active: false, is_uploading_completed: true, uploaded_size: 57569 } } }
    log::info!("Downloading {} id {}", file_type, file_id);
    while let Some(message) = receiver.recv().await {
        if let Update::File(filemess) = message.borrow() {
            io::stdout().flush().unwrap();
            if filemess.file().local().is_downloading_completed() {
                log::debug!("file message received: {:?}", filemess);
                let new_filename = format!(
                    "output/{}--{}_Telegram_{}_{}",
                    date_time.format("%Y-%m-%d_%H-%M-%S"),
                    file_type,
                    slugify(chat_title),
                    Path::new(filemess.file().local().path())
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                );
                log::info!(
                    "Moving file from {} to {}",
                    filemess.file().local().path(),
                    new_filename
                );
                fs::rename(filemess.file().local().path(), new_filename).unwrap();
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let mut days_back: i64 = 9999;

    if args.len() > 2 {
        panic!("Only takes one optional argument, the number of days back to process images");
    }

    if args.len() == 2 {
        days_back = args[1].parse().unwrap();
    }

    let settings = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("Settings"))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        .add_source(config::Environment::with_prefix(""))
        .build()
        .unwrap();

    tdjson::set_log_verbosity_level(1);
    // Log info by default
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Box<Update>>(10000);

    let client = Client::builder()
        .with_tdlib_parameters(
            TdlibParameters::builder()
                .database_directory("telegram_database")
                .use_test_dc(false)
                .api_id(i32::try_from(settings.get_int("api_id").unwrap()).unwrap())
                .api_hash(settings.get_string("api_hash").unwrap())
                .system_language_code("en")
                .device_model("Desktop")
                .system_version("Unknown")
                .application_version(env!("CARGO_PKG_VERSION"))
                .enable_storage_optimizer(true)
                .build(),
        )
        .with_updates_sender(sender)
        .build()
        .unwrap();

    let auth_handler = ConsoleAndEmptyAuthStateHandler::new();
    let mut worker = Worker::builder()
        .with_auth_state_handler(auth_handler)
        .build()
        .unwrap();

    worker.start();

    let client = worker.bind_client(client).await.unwrap();

    loop {
        if worker.wait_client_state(&client).await.unwrap() == ClientState::Opened {
            log::info!("client authorized");
            break;
        }
    }
    let me1 = client.get_me(GetMe::builder().build()).await.unwrap();
    log::info!("client info: {:?}", me1);

    // Get the complete list of chats we're in
    let chats = client
        .get_chats(GetChats::builder().limit(100).build())
        .await
        .unwrap();

    for chat in chats.chat_ids().iter() {
        let chat_info = client
            .get_chat(GetChat::builder().chat_id(chat.to_owned()).build())
            .await
            .unwrap();
        log::info!("Working on chat {}", chat_info.title());
        let mut earliest_message_id: i64 = 0;
        let date_since = Utc::now() - Duration::days(days_back);
        'outer: loop {
            let history = client
                .get_chat_history(
                    GetChatHistory::builder()
                        .limit(50)
                        .chat_id(chat.to_owned())
                        .from_message_id(earliest_message_id)
                        .build(),
                )
                .await
                .unwrap();
            if history.messages().is_empty() {
                break;
            }
            // println!("emid: {}", earliest_message_id);
            for message in history.messages().iter().flatten() {
                let date_time = Pacific.timestamp_opt(i64::from(message.date()), 0).unwrap();

                if date_time < date_since {
                    log::info!("Found a message with date {}, which is older than we're looking for, so stopping with chat {}", date_time.format("%Y-%d-%m %H:%M %Z"), chat_info.title());
                    break 'outer;
                }

                if earliest_message_id == 0 {
                    earliest_message_id = message.id()
                }
                if message.id() < earliest_message_id {
                    earliest_message_id = message.id()
                }

                // println!("date: {}", date_time);

                match message.content() {
                    MessageContent::MessageAnimation(animation) => {
                        // println!("{:#?}", animation.animation());

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            animation.animation().animation().id(),
                            "MOV",
                        )
                        .await;
                    }
                    MessageContent::MessageAudio(audio) => {
                        // println!("{:#?}", audio.audio());

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            audio.audio().audio().id(),
                            "Audio",
                        )
                        .await;
                    }
                    MessageContent::MessageDocument(document) => {
                        // println!("{:#?}", document.document());

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            document.document().document().id(),
                            "File",
                        )
                        .await;
                    }
                    MessageContent::MessagePhoto(photo) => {
                        // println!("{:#?}", photo.photo());

                        let mut photo_id = 0;
                        let mut photo_size = 0;
                        for size in photo.photo().sizes().iter() {
                            // println!("{:#?}", size.photo().id());
                            // println!("{:#?}", size.photo().size());

                            // Pick the biggest
                            if size.photo().size() > photo_size {
                                photo_size = size.photo().size();
                                photo_id = size.photo().id();
                            }
                        }

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            photo_id,
                            "IMG",
                        )
                        .await;
                    }
                    MessageContent::MessageVideo(video) => {
                        // println!("{:#?}", video.video());

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            video.video().video().id(),
                            "MOV",
                        )
                        .await;
                    }
                    MessageContent::MessageVideoNote(video_note) => {
                        // println!("{:#?}", video_note.video_note());

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            video_note.video_note().video().id(),
                            "MOV",
                        )
                        .await;
                    }
                    MessageContent::MessageVoiceNote(voice_note) => {
                        // println!("{:#?}", voice_note.voice_note());

                        my_download_file(
                            &client,
                            &mut receiver,
                            date_time,
                            chat_info.title(),
                            voice_note.voice_note().voice().id(),
                            "Audio",
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        }
    }

    client.stop().await.unwrap();

    loop {
        if worker.wait_client_state(&client).await.unwrap() == ClientState::Closed {
            log::info!("client closed");
            break;
        }
    }
    worker.stop();
    log::info!("worker stopped");
}
