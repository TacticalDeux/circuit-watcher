use chrono::prelude::*;
use futures::SinkExt;
use futures_util::stream::StreamExt;
use http::{header::AUTHORIZATION, HeaderValue};
use league_client_connector::LeagueClientConnector;
use regex::Regex;
use reqwest::{header, ClientBuilder};
use std::error::Error;
use std::io::Write;
use sysinfo::{self, ProcessExt, System, SystemExt};
use tungstenite::{client::IntoClientRequest, protocol::WebSocketConfig};

macro_rules! timestamped_println {
    ($($arg:tt)*) => {
        {
            let current_time = Local::now();
            let formatted_time = current_time.format("[%Y-%m-%d - %H:%M]");
            println!("{}: {}", formatted_time, format_args!($($arg)*));
        }
    }
}
macro_rules! timestamped_print {
    ($($arg:tt)*) => {
        {
            let current_time = Local::now();
            let formatted_time = current_time.format("[%Y-%m-%d - %H:%M]");
            print!("\r{} {}", formatted_time, format_args!($($arg)*));
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let process_pattern = "LeagueClient";
    let mut system = System::new_all();
    let program_control_thread = tokio::spawn(key_listener());

    println!("Press the END key to terminate the program.");
    while !(find_processes_by_regex(&mut system, process_pattern).await) {
        if program_control_thread.is_finished() {
            return Ok(());
        }
        println!("The pattern '{process_pattern}' does not match any active processes. You may have closed the LoL client.\nRetrying in 30 seconds.");
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }

    let lockfile = LeagueClientConnector::parse_lockfile().unwrap();
    let auth_header =
        HeaderValue::from_str(format!("Basic {}", lockfile.b64_auth).as_str()).unwrap();
    let go_to_connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .unwrap();
    let connfig = WebSocketConfig::default();
    let connector = tokio_tungstenite::Connector::NativeTls(go_to_connector);
    let mut request = format!("wss://127.0.0.1:{}/", lockfile.port)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert(AUTHORIZATION, auth_header.clone());
    let (mut socket, _) = tokio_tungstenite::connect_async_tls_with_config(
        request,
        Some(connfig),
        false,
        Some(connector),
    )
    .await
    .unwrap();
    timestamped_println!(
        "Connected to LoL client's WSS at wss://{}:{} with auth: base64:{}\n",
        lockfile.address,
        lockfile.port,
        lockfile.b64_auth
    );
    let cert = reqwest::Certificate::from_pem(include_bytes!("../riotgames.pem")).unwrap();
    let mut headers = header::HeaderMap::new();

    headers.insert(AUTHORIZATION, auth_header.clone());
    let rest_client = ClientBuilder::new()
        .add_root_certificate(cert)
        .default_headers(headers)
        .build()
        .unwrap();

    socket
        .send(tungstenite::Message::Text(
            "[5, \"OnJsonApiEvent\"]".to_string(),
        ))
        .await
        .unwrap();

    let mut dots_count = 0;
    let mut found_match = false;
    loop {
        if program_control_thread.is_finished() {
            break;
        }

        while let Some(Ok(event)) = socket.next().await {
            if program_control_thread.is_finished() {
                return Ok(());
            }
            if event.to_string().contains("\"searchState\":\"Searching\"") {
                dots_count = (dots_count + 1) % 4;
                timestamped_print!("Searching for a match");
                for _ in 0..dots_count {
                    print!(".");
                }
                for _ in dots_count..3 {
                    print!(" ");
                }
                std::io::stdout().flush().unwrap();
                found_match = false;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            if event
                .to_string()
                .contains("\"playerResponse\":\"None\",\"state\":\"InProgress\"")
                && !found_match
            {
                found_match = true;
                timestamped_println!("\nMatch found, accepting.");
                std::io::stdout().flush().unwrap();
                rest_client
                    .post(format!(
                        "https://127.0.0.1:{}/lol-matchmaking/v1/ready-check/accept",
                        lockfile.port
                    ))
                    .send()
                    .await?;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }
    }

    Ok(())
}

async fn find_processes_by_regex(system: &mut System, process_pattern: &str) -> bool {
    system.refresh_all();
    let regex = Regex::new(process_pattern).unwrap();
    let mut process_exists = false;

    for process in system.processes().values() {
        if regex.is_match(process.name()) {
            process_exists = true;
        }
    }
    process_exists
}

async fn key_listener() {
    use winapi::um::winuser::GetAsyncKeyState;
    let pressed = -32767;
    loop {
        let end_key = unsafe {
            GetAsyncKeyState(0x23 /*VK_END*/)
        };

        if end_key == pressed {
            timestamped_println!("\nEND key pressed, terminating the program.");
            break;
        }
    }
}
