use std::thread;

use ashpd::{
    AppID,
    desktop::{
        CreateSessionOptions,
        global_shortcuts::{
            BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, ListShortcutsOptions,
            NewShortcut, Shortcut,
        },
    },
};
use futures_util::StreamExt;
use ksni::{Tray, TrayMethods, menu::StandardItem};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    app::{self, AppAction},
    capture::CaptureMode,
    error::{BlinkError, Result},
    notification,
    ui::UiMessage,
};

pub const APP_ID: &str = "dev.blink.Blink";

struct BlinkTray {
    sender: UnboundedSender<AppAction>,
}

impl Tray for BlinkTray {
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        APP_ID.to_owned()
    }

    fn title(&self) -> String {
        "Blink".to_owned()
    }

    fn icon_name(&self) -> String {
        "camera-photo".to_owned()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        queue_action(&self.sender, AppAction::ShowWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            menu_item("Open Blink", AppAction::ShowWindow),
            ksni::MenuItem::Separator,
            menu_item("Capture Area", AppAction::Capture(CaptureMode::Area)),
            menu_item("Capture Screen", AppAction::Capture(CaptureMode::Screen)),
            menu_item("Capture Window", AppAction::Capture(CaptureMode::Window)),
            ksni::MenuItem::Separator,
            menu_item("Open Screenshots Folder", AppAction::OpenFolder),
            menu_item("Quit", AppAction::Quit),
        ]
    }
}

fn menu_item(label: &str, action: AppAction) -> ksni::MenuItem<BlinkTray> {
    StandardItem {
        label: label.to_owned(),
        activate: Box::new(move |tray: &mut BlinkTray| queue_action(&tray.sender, action)),
        ..Default::default()
    }
    .into()
}

fn queue_action(sender: &UnboundedSender<AppAction>, action: AppAction) {
    if let Err(error) = sender.send(action) {
        eprintln!("Blink: action could not be queued: {error}");
    }
}

pub fn start_worker(
    sender: UnboundedSender<AppAction>,
    receiver: UnboundedReceiver<AppAction>,
    ui_sender: std::sync::mpsc::Sender<UiMessage>,
) -> Result<()> {
    thread::Builder::new()
        .name("blink-worker".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ui_sender.send(UiMessage::Status(format!(
                        "Could not start Blink services: {error}"
                    )));
                    return;
                }
            };
            runtime.block_on(run(sender, receiver, ui_sender));
        })
        .map(|_| ())
        .map_err(BlinkError::Worker)
}

async fn run(
    sender: UnboundedSender<AppAction>,
    mut receiver: UnboundedReceiver<AppAction>,
    ui_sender: std::sync::mpsc::Sender<UiMessage>,
) {
    register_host_application().await;

    let tray_handle = match (BlinkTray {
        sender: sender.clone(),
    })
    .spawn()
    .await
    {
        Ok(handle) => Some(handle),
        Err(error) => {
            eprintln!("Blink: system tray is unavailable: {error}");
            None
        }
    };

    let (shortcut_sender, shortcut_receiver) = tokio::sync::mpsc::unbounded_channel();
    let shortcuts_ui_sender = ui_sender.clone();
    let shortcuts_action_sender = sender.clone();
    tokio::spawn(async move {
        if let Err(error) = run_shortcuts(
            shortcuts_action_sender,
            shortcut_receiver,
            shortcuts_ui_sender.clone(),
        )
        .await
        {
            eprintln!("Blink: global shortcuts unavailable: {error}");
            let _ = shortcuts_ui_sender.send(UiMessage::Status(
                "Global shortcuts are unavailable on this desktop".to_owned(),
            ));
        }
    });

    while let Some(action) = receiver.recv().await {
        match action {
            AppAction::Capture(mode) => {
                if let Err(error) = app::capture(mode, true).await
                    && !error.is_cancelled()
                {
                    eprintln!("Blink: {error}");
                }
            }
            AppAction::ShowWindow => {
                let _ = ui_sender.send(UiMessage::ShowWindow);
            }
            AppAction::OpenFolder => {
                if let Err(error) = app::open_screenshots_folder().await {
                    report_error(&ui_sender, &error).await;
                }
            }
            AppAction::ConfigureShortcuts => {
                if shortcut_sender.send(()).is_err() {
                    let _ = ui_sender.send(UiMessage::Status(
                        "Global shortcut service is unavailable".to_owned(),
                    ));
                }
            }
            AppAction::Quit => {
                let _ = ui_sender.send(UiMessage::Quit);
                break;
            }
        }
    }

    if let Some(handle) = tray_handle {
        handle.shutdown().await;
    }
}

async fn register_host_application() {
    let app_id = match APP_ID.parse::<AppID>() {
        Ok(app_id) => app_id,
        Err(error) => {
            eprintln!("Blink: invalid application ID: {error}");
            return;
        }
    };
    if let Err(error) = ashpd::register_host_app(app_id).await {
        eprintln!("Blink: could not register portal application identity: {error}");
    }
}

async fn run_shortcuts(
    action_sender: UnboundedSender<AppAction>,
    mut configure_receiver: UnboundedReceiver<()>,
    ui_sender: std::sync::mpsc::Sender<UiMessage>,
) -> std::result::Result<(), ashpd::Error> {
    let portal = GlobalShortcuts::new().await?;
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await?;

    let previous = portal
        .list_shortcuts(&session, ListShortcutsOptions::default())
        .await?
        .response()?;
    let mut bound = !previous.shortcuts().is_empty();
    if bound {
        send_shortcut_labels(&ui_sender, previous.shortcuts());
    } else {
        let _ = ui_sender.send(UiMessage::Status(
            "Set up global shortcuts when you're ready".to_owned(),
        ));
    }

    let mut activated = portal.receive_activated().await?;
    let mut changed = portal.receive_shortcuts_changed().await?;
    loop {
        tokio::select! {
            Some(event) = activated.next() => {
                if let Some(mode) = mode_for_shortcut(event.shortcut_id()) {
                    queue_action(&action_sender, AppAction::Capture(mode));
                }
            }
            Some(event) = changed.next() => {
                send_shortcut_labels(&ui_sender, event.shortcuts());
            }
            Some(()) = configure_receiver.recv() => {
                if bound {
                    if let Err(error) = portal.configure_shortcuts(
                        &session,
                        None,
                        ConfigureShortcutsOptions::default(),
                    ).await {
                        let _ = ui_sender.send(UiMessage::Status(format!(
                            "Could not configure shortcuts: {error}"
                        )));
                    }
                } else {
                    match portal.bind_shortcuts(
                        &session,
                        &default_shortcuts(),
                        None,
                        BindShortcutsOptions::default(),
                    ).await.and_then(|request| request.response()) {
                        Ok(response) => {
                            bound = true;
                            send_shortcut_labels(&ui_sender, response.shortcuts());
                        }
                        Err(error) => {
                            let _ = ui_sender.send(UiMessage::Status(format!(
                                "Shortcut setup was not completed: {error}"
                            )));
                        }
                    }
                }
            }
            else => break,
        }
    }
    Ok(())
}

fn default_shortcuts() -> [NewShortcut; 3] {
    [
        NewShortcut::new("capture-area", "Capture an area").preferred_trigger("Print"),
        NewShortcut::new("capture-screen", "Capture the screen").preferred_trigger("SHIFT+Print"),
        NewShortcut::new("capture-window", "Capture a window").preferred_trigger("ALT+Print"),
    ]
}

fn mode_for_shortcut(id: &str) -> Option<CaptureMode> {
    match id {
        "capture-area" => Some(CaptureMode::Area),
        "capture-screen" => Some(CaptureMode::Screen),
        "capture-window" => Some(CaptureMode::Window),
        _ => None,
    }
}

fn send_shortcut_labels(ui_sender: &std::sync::mpsc::Sender<UiMessage>, shortcuts: &[Shortcut]) {
    let labels = shortcuts
        .iter()
        .map(|shortcut| {
            (
                shortcut.id().to_owned(),
                shortcut.trigger_description().to_owned(),
            )
        })
        .collect();
    let _ = ui_sender.send(UiMessage::Shortcuts(labels));
}

async fn report_error(ui_sender: &std::sync::mpsc::Sender<UiMessage>, error: &BlinkError) {
    eprintln!("Blink: {error}");
    let _ = ui_sender.send(UiMessage::Status(error.to_string()));
    if let Err(notification_error) = notification::error(error).await {
        eprintln!("Blink: notification failed: {notification_error}");
    }
}
