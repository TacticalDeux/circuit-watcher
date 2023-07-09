use eframe::egui;
use egui::TextEdit;
use http::{header::AUTHORIZATION, HeaderValue};
use league_client_connector::LeagueClientConnector;
use reqwest::{header, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Debug)]
pub struct GUI {
    pick_ban_selection: Arc<AtomicBool>,
    rune_page_selection: Arc<AtomicBool>,
    auto_accept: Arc<AtomicBool>,
    pick_text: String,
    ban_text: String,
    text: String,
    champion_picks: Arc<Mutex<Vec<(u32, String)>>>,
    ban_picks: Arc<Mutex<Option<(u32, String)>>>,
    champions: Vec<Champion>,
    gameflow_status: Arc<Mutex<String>>,

    connection_status: Arc<Mutex<Option<String>>>,

    clear_label_timer: Option<std::time::Instant>,
    pick_not_found_label_timer: Option<std::time::Instant>,
    ban_not_found_label_timer: Option<std::time::Instant>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
/// The `Champion` struct is a data structure used for (de)serialization of the `champsions.json` file.
///
/// ### Properties:
/// * `id`: The `id` property is of type `u32`, which stands for "unsigned 32-bit integer". It is used
/// to uniquely identify each instance of the `Champion` struct.
/// * `name`: The `name` property is a string that represents the name of a champion.
struct Champion {
    id: u32,
    name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
/// The `ActionResponseData` struct is a data structure used to represent the response data for a champion select action.
///
/// ### Properties:
/// * `actorCellId`: The `actorCellId` property is of type `i32`, which stands for a 32-bit signed
/// integer. It represents the ID of a summoner in the given champion selection lobby.
/// * `completed`: The "completed" property is a boolean value that indicates whether the action
/// associated with the response data has been completed or not.
/// * `id`: The `id` property is of type `i32`, which stands for a 32-bit signed integer. It is used to
/// uniquely identify an action response data object. It differs from the `actorCellId` by being a unique id tied to the action `r#type`.
/// * `isInProgress`: The `isInProgress` property is a boolean value that indicates whether the action
/// is currently in progress or not.
/// * `r#type`: The property "r#type" is a string that represents the type of action response data. The
/// "r#" prefix is used to escape the reserved keyword "type" in Rust.
struct ActionResponseData {
    actorCellId: i32,
    completed: bool,
    id: i32,
    isInProgress: bool,
    r#type: String,
}

impl GUI {
    fn new(/*cc: &eframe::CreationContext<'_>*/) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.

        // Initialize checkbox states
        let pick_ban_selection = Arc::new(AtomicBool::new(false));
        let rune_page_selection = Arc::new(AtomicBool::new(false));
        let auto_accept = Arc::new(AtomicBool::new(false));
        let connection_status = Arc::new(Mutex::new(None));
        let json_data = std::fs::read_to_string("champions.json").expect("Failed to read file");
        let champions: Vec<Champion> =
            serde_json::from_str(&json_data).expect("Failed to parse JSON");

        Self {
            pick_ban_selection,
            rune_page_selection,
            auto_accept,
            pick_text: String::new().to_owned(),
            ban_text: String::new().to_owned(),
            champion_picks: Arc::new(Mutex::new(Vec::new())),
            ban_picks: Arc::new(Mutex::new(None)),
            clear_label_timer: None,
            pick_not_found_label_timer: None,
            ban_not_found_label_timer: None,
            connection_status,
            champions,
            text: String::new().to_owned(),
            gameflow_status: Arc::new(Mutex::new(String::new())),
        }
    }
}

impl eframe::App for GUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let pick_ban_selection = self.pick_ban_selection.load(Ordering::SeqCst);
        if let Some(timer) = self.clear_label_timer {
            let elapsed = timer.elapsed();
            if elapsed.as_secs_f32() > 3.0 {
                self.clear_label_timer = None;
            }
        }
        if let Some(timer) = self.pick_not_found_label_timer {
            let elapsed = timer.elapsed();
            if elapsed.as_secs_f32() > 1.5 {
                self.pick_not_found_label_timer = None;
            }
        }
        if let Some(timer) = self.ban_not_found_label_timer {
            let elapsed = timer.elapsed();
            if elapsed.as_secs_f32() > 1.5 {
                self.ban_not_found_label_timer = None;
            }
        }
        let mut champion_picks = self.champion_picks.lock().unwrap();
        let mut ban_picks = self.ban_picks.lock().unwrap();
        let connection_status = self.connection_status.lock().unwrap();
        let gameflow_status = self.gameflow_status.lock().unwrap();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Circuit Watcher");

            ui.collapsing("App Settings", |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Clear Picks/Bans").clicked() {
                        champion_picks.clear();
                        *ban_picks = None;
                        self.clear_label_timer = Some(std::time::Instant::now());
                    }
                    if self.clear_label_timer.is_some() {
                        ui.strong("Picks and bans cleared.");
                    }
                });

                ui.horizontal(|ui| {
                    let auto_accept_label = if self.auto_accept.load(Ordering::SeqCst) {
                        "Auto Accept: ON"
                    } else {
                        "Auto Accept: OFF"
                    };

                    if ui
                        .checkbox(
                            &mut self.auto_accept.load(Ordering::SeqCst),
                            auto_accept_label,
                        )
                        .clicked()
                    {
                        let current_state = self.auto_accept.load(Ordering::SeqCst);
                        self.auto_accept.store(!current_state, Ordering::SeqCst);
                    }
                });

                // TODO:
                // ui.horizontal(|ui| {
                //     let rune_page_label = if self.rune_page_selection.load(Ordering::SeqCst) {
                //         "Rune Page Change: ON"
                //     } else {
                //         "Rune Page Change: OFF"
                //     };

                //     if ui
                //         .checkbox(
                //             &mut self.rune_page_selection.load(Ordering::SeqCst),
                //             rune_page_label,
                //         )
                //         .clicked()
                //     {
                //         let current_state = self.rune_page_selection.load(Ordering::SeqCst);
                //         self.rune_page_selection
                //             .store(!current_state, Ordering::SeqCst);
                //     }
                // });

                ui.horizontal(|ui| {
                    let pick_ban_label = if self.pick_ban_selection.load(Ordering::SeqCst) {
                        "Auto-Pick/Ban: ON"
                    } else {
                        "Auto-Pick/Ban: OFF"
                    };

                    if ui
                        .checkbox(
                            &mut self.pick_ban_selection.load(Ordering::SeqCst),
                            pick_ban_label,
                        )
                        .clicked()
                    {
                        let current_state = self.pick_ban_selection.load(Ordering::SeqCst);
                        self.pick_ban_selection
                            .store(!current_state, Ordering::SeqCst);
                    }
                });

                ui.vertical(|ui| {
                    if pick_ban_selection {
                        if champion_picks.len() < 2 {
                            ui.label("Enter champions to pick (2 max):");
                            let text_edit_picks = ui.add(
                                TextEdit::singleline(&mut self.pick_text)
                                    .hint_text("Press enter to skip."),
                            );
                            if text_edit_picks.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                let pick_text_cleaned = self
                                    .pick_text
                                    .trim()
                                    .replace(" ", "")
                                    .as_str()
                                    .replace("'", "")
                                    .to_lowercase();

                                let matching_champion = self.champions.iter().find(|champion| {
                                    champion.name.to_lowercase() == pick_text_cleaned
                                });

                                if !pick_text_cleaned.is_empty() {
                                    match matching_champion {
                                        Some(champion) => {
                                            if champion_picks
                                                .contains(&(champion.id, champion.name.clone()))
                                            {
                                                self.text = "Champion has alread been selected."
                                                    .to_string();
                                                self.pick_not_found_label_timer =
                                                    Some(std::time::Instant::now());
                                            } else {
                                                champion_picks
                                                    .push((champion.id, champion.name.clone()));
                                            }
                                        }
                                        None => {
                                            self.text = "No champion found with the given name."
                                                .to_string();
                                            self.pick_not_found_label_timer =
                                                Some(std::time::Instant::now());
                                        }
                                    }
                                } else {
                                    champion_picks.push((0, "".to_string()));
                                }
                                self.pick_text.clear();
                                text_edit_picks.request_focus();
                            }
                            if self.pick_not_found_label_timer.is_some() {
                                ui.weak(&self.text);
                            }
                        }

                        if ban_picks.is_none() {
                            ui.label("Enter champion to ban:");
                            let text_edit_bans = ui.add(
                                TextEdit::singleline(&mut self.ban_text)
                                    .hint_text("Press enter to skip."),
                            );

                            if text_edit_bans.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                let ban_text_cleaned = self
                                    .ban_text
                                    .trim()
                                    .replace(" ", "")
                                    .as_str()
                                    .replace("'", "")
                                    .to_lowercase();

                                let matching_champion = self.champions.iter().find(|champion| {
                                    champion.name.to_lowercase() == ban_text_cleaned
                                });

                                if !ban_text_cleaned.is_empty() {
                                    match matching_champion {
                                        Some(champion) => {
                                            if champion_picks
                                                .contains(&(champion.id, champion.name.clone()))
                                            {
                                                self.text = "Champion has alread been selected."
                                                    .to_string();
                                                self.ban_not_found_label_timer =
                                                    Some(std::time::Instant::now());
                                            } else {
                                                *ban_picks =
                                                    Some((champion.id, champion.name.clone()));
                                            }
                                        }
                                        None => {
                                            self.text = "No champion found with the given name."
                                                .to_string();
                                            self.ban_not_found_label_timer =
                                                Some(std::time::Instant::now());
                                        }
                                    }
                                } else {
                                    *ban_picks = Some((
                                        0,
                                        self.ban_text
                                            .trim()
                                            .replace(" ", "")
                                            .as_str()
                                            .replace("'", "")
                                            .to_string()
                                            .to_lowercase(),
                                    ));
                                }
                                self.ban_text.clear();
                                text_edit_bans.request_focus();
                            }
                            if self.ban_not_found_label_timer.is_some() {
                                ui.weak(&self.text);
                            }
                        }
                    }
                    if pick_ban_selection {
                        if champion_picks.len() == 2
                            && champion_picks.get(0).unwrap().1.is_empty()
                            && ban_picks.is_some()
                            && ban_picks.as_ref().unwrap().1.is_empty()
                            && champion_picks.get(1).unwrap().1.is_empty()
                        {
                            champion_picks.clear();
                            *ban_picks = None;
                            self.pick_ban_selection.store(false, Ordering::SeqCst);
                        }
                        if champion_picks.len() != 0 {
                            ui.strong("Picks:");
                            for (id, name) in &*champion_picks {
                                if !name.is_empty() {
                                    ui.label(format!("ID:{id} Name:\"{name}\""));
                                } else {
                                    ui.label("None");
                                }
                            }
                        }
                        if ban_picks.is_some() {
                            ui.strong("Ban:");
                            if ban_picks.as_ref().unwrap().1.is_empty() {
                                ui.label("None");
                            } else {
                                ui.label(format!(
                                    "ID:{} Name:\"{}\"",
                                    &ban_picks.as_ref().unwrap().0,
                                    &ban_picks.as_ref().unwrap().1
                                ));
                            }
                        }
                    }
                });
            });

            ui.collapsing("Match State", |ui| {
                ui.heading(format!("{}", gameflow_status.clone()));
            });

            ui.vertical_centered_justified(|ui| {
                ui.add_space(ui.available_size().y - ui.spacing().item_spacing.y * 2.0);
                if let Some(status) = connection_status.clone() {
                    ui.weak(status.clone());
                }
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        std::process::exit(0);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(500.0, 400.0)),
        ..Default::default()
    };

    let app = GUI::new();

    let champion_picks_clone = Arc::clone(&app.champion_picks);
    let ban_picks_clone = Arc::clone(&app.ban_picks);
    let connection_status = Arc::clone(&app.connection_status);
    let connection_status_clone = Arc::clone(&app.connection_status);
    let gameflow_status = Arc::clone(&app.gameflow_status);
    let pick_ban_selection_clone = Arc::clone(&app.pick_ban_selection);
    let rune_page_change_clone = Arc::clone(&app.rune_page_selection);
    let auto_accept_clone = Arc::clone(&app.auto_accept);

    tokio::spawn(async move {
        loop {
            hide_console_window();
            match LeagueClientConnector::parse_lockfile() {
                Ok(lockfile) => {
                    let mut status = connection_status.lock().unwrap();
                    *status = Some(format!(
                        "Connected to LeagueClient on https://127.0.0.1:{}",
                        lockfile.port
                    ));
                }
                Err(_) => {
                    let mut status = connection_status.lock().unwrap();
                    *status = Some("LeagueClient not found, may be closed.".to_owned());
                }
            }
        }
    });

    tokio::spawn(async move {
        let status = connection_status_clone.lock().unwrap().clone();

        // Both of this while loops are to ensure there is a viable connection to the League Client
        while status.is_none() {
            if connection_status_clone.lock().unwrap().clone().is_some() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
        }
        while connection_status_clone
            .lock()
            .unwrap()
            .clone()
            .as_ref()
            .unwrap()
            .contains("LeagueClient not found, may be closed.")
        {
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        let mut lockfile = LeagueClientConnector::parse_lockfile().unwrap();
        let mut auth_header =
            HeaderValue::from_str(format!("Basic {}", lockfile.b64_auth).as_str()).unwrap();
        let cert = reqwest::Certificate::from_pem(include_bytes!("../riotgames.pem")).unwrap();
        let mut headers = header::HeaderMap::new();

        headers.insert(AUTHORIZATION, auth_header.clone());
        let mut rest_client = ClientBuilder::new()
            .add_root_certificate(cert.clone())
            .default_headers(headers)
            .build()
            .unwrap();

        let mut locked_champ = false;
        loop {
            if connection_status_clone
                .lock()
                .unwrap()
                .clone()
                .as_ref()
                .unwrap()
                .contains("LeagueClient not found, may be closed.")
            {
                match LeagueClientConnector::parse_lockfile() {
                    Ok(riotlockfile) => {
                        lockfile = riotlockfile;
                        auth_header =
                            HeaderValue::from_str(format!("Basic {}", lockfile.b64_auth).as_str())
                                .unwrap();
                        headers = header::HeaderMap::new();

                        headers.insert(AUTHORIZATION, auth_header.clone());
                        rest_client = ClientBuilder::new()
                            .add_root_certificate(cert.clone())
                            .default_headers(headers)
                            .build()
                            .unwrap();

                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    }
                    Err(_) => {
                        continue;
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }

            let champion_picks = champion_picks_clone.lock().unwrap().clone();
            let ban_picks = ban_picks_clone.lock().unwrap().clone();
            let gameflow_status_clone = Arc::clone(&gameflow_status);
            let pick_ban_selection = pick_ban_selection_clone.load(Ordering::SeqCst);
            let rune_change = rune_page_change_clone.load(Ordering::SeqCst);
            let auto_accept = auto_accept_clone.load(Ordering::SeqCst);

            let gameflow: serde_json::Value = rest_client
                .get(format!(
                    "https://127.0.0.1:{}/lol-gameflow/v1/session",
                    lockfile.port
                ))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            let phase = gameflow["phase"].as_str();

            match phase {
                Some("Matchmaking") => {
                    *gameflow_status_clone.lock().unwrap() = "Looking for a match".to_owned();
                    locked_champ = false;
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                }
                Some("Lobby") => {
                    *gameflow_status_clone.lock().unwrap() = "In Lobby".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                }
                Some("ReadyCheck") => {
                    if auto_accept {
                        *gameflow_status_clone.lock().unwrap() = "Accepting match".to_owned();
                        rest_client
                            .post(format!(
                                "https://127.0.0.1:{}/lol-matchmaking/v1/ready-check/accept",
                                lockfile.port
                            ))
                            .send()
                            .await
                            .unwrap();
                        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    }
                    *gameflow_status_clone.lock().unwrap() = "Match Found".to_owned();
                }
                Some("ChampSelect") => {
                    if pick_ban_selection {
                        *gameflow_status_clone.lock().unwrap() =
                            "Champion Selection with Auto-pick/ban ON".to_owned();

                        if champion_picks.len() == 0 && ban_picks.is_none() {
                            continue;
                        }

                        let current_champ_select: serde_json::Value = rest_client
                            .get(format!(
                                "https://127.0.0.1:{}/lol-champ-select/v1/session",
                                lockfile.port
                            ))
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();

                        let action_response: Vec<Vec<ActionResponseData>> =
                            serde_json::from_value(current_champ_select["actions"].clone())
                                .unwrap();
                        let filtered_action_data: Vec<ActionResponseData> = action_response
                            .iter()
                            .flatten()
                            .filter(|data| {
                                data.actorCellId == current_champ_select["localPlayerCellId"]
                            })
                            .take(2) // Limit to a maximum of 2 matches
                            .cloned()
                            .collect();
                        let extracted_action_data: Vec<(i32, bool, String, bool)> =
                            filtered_action_data
                                .iter()
                                .map(|data| {
                                    (
                                        data.id,
                                        data.isInProgress,
                                        data.r#type.clone(),
                                        data.completed,
                                    )
                                })
                                .collect();

                        let (ban_id, ban_is_in_progress, _type1, ban_completed) =
                            extracted_action_data.get(0).cloned().unwrap_or((
                                0,
                                false,
                                "".to_string(),
                                false,
                            ));
                        let (pick_id, pick_is_in_progress, _type2, pick_completed) =
                            extracted_action_data.get(1).cloned().unwrap_or((
                                0,
                                false,
                                "".to_string(),
                                false,
                            ));

                        if ban_picks.is_some() {
                            if !ban_picks.as_ref().unwrap().1.is_empty() {
                                let ban_body = serde_json::json!({
                                        "actorCellId": current_champ_select["localPlayerCellId"],
                                        "championId": &ban_picks.as_ref().unwrap().0,
                                        "completed": true,
                                        "id": &ban_id,
                                        "isAllyAction": true,
                                        "type": "ban"
                                });
                                let ban_champ_info: serde_json::Value = rest_client
                                    .get(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                                    lockfile.port,
                                    &ban_picks.as_ref().unwrap().0
                                ))
                                    .send()
                                    .await
                                    .unwrap()
                                    .json()
                                    .await
                                    .unwrap();

                                if ban_is_in_progress
                                    && !ban_completed
                                    && ban_champ_info["selectionStatus"]["pickedByOtherOrBanned"]
                                        != true
                                    && current_champ_select["timer"]["phase"] != "PLANNING"
                                {
                                    rest_client
                                        .patch(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                    lockfile.port, ban_id
                                ))
                                        .json(&ban_body)
                                        .send()
                                        .await
                                        .unwrap();
                                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                                }
                            }
                        }

                        if champion_picks.len() != 0 {
                            if champion_picks.get(0).unwrap().1.is_empty()
                                && champion_picks.get(1).unwrap().1.is_empty()
                            {
                                continue;
                            }
                            if !champion_picks.get(0).unwrap().1.is_empty() {
                                let pick_champ_info: serde_json::Value = rest_client
                                    .get(format!(
                                "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                                lockfile.port, champion_picks.get(0).unwrap().0
                            ))
                                    .send()
                                    .await
                                    .unwrap()
                                    .json()
                                    .await
                                    .unwrap();

                                let pick_body = serde_json::json!({
                                        "actorCellId": current_champ_select["localPlayerCellId"],
                                        "championId": champion_picks.get(0).unwrap().0,
                                        "completed": true,
                                        "id": &pick_id,
                                        "isAllyAction": true,
                                        "type": "pick"
                                });

                                if !pick_is_in_progress
                                    && pick_completed
                                    && !ban_is_in_progress
                                    && ban_completed
                                    || current_champ_select["timer"]["phase"] == "PLANNING"
                                {
                                    continue;
                                }

                                if !pick_is_in_progress {
                                    continue;
                                }
                                if pick_champ_info["selectionStatus"]["pickedByOtherOrBanned"]
                                    != true
                                {
                                    if pick_is_in_progress
                                        && !pick_completed
                                        && !ban_is_in_progress
                                        && ban_completed
                                        && pick_champ_info["selectionStatus"]
                                            ["pickedByOtherOrBanned"]
                                            != true
                                        && !locked_champ
                                    {
                                        if rune_change {
                                            // TODO:
                                        }
                                        rest_client
                                            .patch(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                    lockfile.port, pick_id
                                ))
                                            .json(&pick_body)
                                            .send()
                                            .await
                                            .unwrap();
                                        locked_champ = true;
                                        tokio::time::sleep(tokio::time::Duration::from_secs(10))
                                            .await;
                                    }
                                }
                            }

                            if !champion_picks.get(1).unwrap().1.is_empty() {
                                let pick_champ_info: serde_json::Value = rest_client
                                    .get(format!(
                                "https://127.0.0.1:{}/lol-champ-select/v1/grid-champions/{}",
                                lockfile.port, champion_picks.get(1).unwrap().0
                            ))
                                    .send()
                                    .await
                                    .unwrap()
                                    .json()
                                    .await
                                    .unwrap();

                                let pick_body = serde_json::json!({
                                        "actorCellId": current_champ_select["localPlayerCellId"],
                                        "championId": champion_picks.get(1).unwrap().0,
                                        "completed": true,
                                        "id": &pick_id,
                                        "isAllyAction": true,
                                        "type": "pick"
                                });

                                if !pick_is_in_progress
                                    && pick_completed
                                    && !ban_is_in_progress
                                    && ban_completed
                                    || current_champ_select["timer"]["phase"] == "PLANNING"
                                {
                                    continue;
                                }

                                if !pick_is_in_progress {
                                    continue;
                                }
                                if pick_champ_info["selectionStatus"]["pickedByOtherOrBanned"]
                                    != true
                                {
                                    if pick_is_in_progress
                                        && !pick_completed
                                        && !ban_is_in_progress
                                        && ban_completed
                                        && pick_champ_info["selectionStatus"]
                                            ["pickedByOtherOrBanned"]
                                            != true
                                        && !locked_champ
                                    {
                                        if rune_change {
                                            // TODO:
                                        }
                                        rest_client
                                            .patch(format!(
                                    "https://127.0.0.1:{}/lol-champ-select/v1/session/actions/{}",
                                    lockfile.port, pick_id
                                ))
                                            .json(&pick_body)
                                            .send()
                                            .await
                                            .unwrap();
                                        locked_champ = true;
                                        tokio::time::sleep(tokio::time::Duration::from_secs(10))
                                            .await;
                                    }
                                }
                            }
                        }
                    }

                    *gameflow_status_clone.lock().unwrap() = "Champion Selection".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                }
                Some("InProgress") => {
                    *gameflow_status_clone.lock().unwrap() = "Game in progress...".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                }
                Some("WaitingForStats") => {
                    *gameflow_status_clone.lock().unwrap() = "Waiting for Stats".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                Some("PreEndOfGame") => {
                    *gameflow_status_clone.lock().unwrap() = "Game in progress...".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                }
                Some("EndOfGame") => {
                    *gameflow_status_clone.lock().unwrap() = "Game Ending...".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                }
                Some(unimplemented_phase) => {
                    *gameflow_status_clone.lock().unwrap() =
                        format!("Unimplemented Phase: {}", unimplemented_phase).to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                }
                None => {
                    *gameflow_status_clone.lock().unwrap() = "Idling...".to_owned();
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                }
            }
        }
    });

    eframe::run_native("Circuit Watcher", options, Box::new(|_cc| Box::new(app)))?;

    Ok(())
}

fn hide_console_window() {
    use std::ptr;
    use winapi::um::wincon::GetConsoleWindow;
    use winapi::um::winuser::{ShowWindow, SW_HIDE};

    let window = unsafe { GetConsoleWindow() };
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
    if window != ptr::null_mut() {
        unsafe {
            ShowWindow(window, SW_HIDE);
        }
    }
}
