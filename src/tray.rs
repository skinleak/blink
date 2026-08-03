use ksni::{Tray, TrayMethods, menu::StandardItem};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    app,
    capture::CaptureMode,
    error::{BlinkError, Result},
    notification,
};

#[derive(Clone, Copy)]
enum Message {
    Capture(CaptureMode),
    OpenFolder,
    Quit,
}

struct BlinkTray {
    sender: UnboundedSender<Message>,
}

impl Tray for BlinkTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "dev.blink.Blink".to_owned()
    }

    fn title(&self) -> String {
        "Blink".to_owned()
    }

    fn icon_name(&self) -> String {
        "camera-photo".to_owned()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            menu_item("Capture Area", Message::Capture(CaptureMode::Area)),
            menu_item("Capture Screen", Message::Capture(CaptureMode::Screen)),
            menu_item("Capture Window", Message::Capture(CaptureMode::Window)),
            ksni::MenuItem::Separator,
            menu_item("Open Screenshots Folder", Message::OpenFolder),
            ksni::MenuItem::Separator,
            menu_item("Quit", Message::Quit),
        ]
    }
}

fn menu_item(label: &str, message: Message) -> ksni::MenuItem<BlinkTray> {
    StandardItem {
        label: label.to_owned(),
        activate: Box::new(move |tray: &mut BlinkTray| {
            if let Err(error) = tray.sender.send(message) {
                eprintln!("Blink: tray action could not be queued: {error}");
            }
        }),
        ..Default::default()
    }
    .into()
}

pub async fn run() -> Result<()> {
    let (sender, mut receiver) = unbounded_channel();
    let handle = BlinkTray { sender }
        .spawn()
        .await
        .map_err(|error| BlinkError::Tray(error.to_string()))?;

    eprintln!("Blink: tray application started");
    event_loop(&mut receiver).await;
    handle.shutdown().await;
    Ok(())
}

async fn event_loop(receiver: &mut UnboundedReceiver<Message>) {
    while let Some(message) = receiver.recv().await {
        match message {
            Message::Capture(mode) => {
                if let Err(error) = app::capture(mode, true).await
                    && !error.is_cancelled()
                {
                    eprintln!("Blink: {error}");
                }
            }
            Message::OpenFolder => {
                if let Err(error) = app::open_screenshots_folder().await {
                    eprintln!("Blink: {error}");
                    if let Err(notification_error) = notification::error(&error).await {
                        eprintln!("Blink: notification failed: {notification_error}");
                    }
                }
            }
            Message::Quit => break,
        }
    }
}
