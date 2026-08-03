use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, HeaderBar, Image,
    Label, MenuButton, Orientation, gio, glib, prelude::*,
};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    app::AppAction,
    capture::CaptureMode,
    error::Result,
    tray::{self, APP_ID},
};

const CSS: &str = r#"
window {
  background: #111318;
  color: #f4f5f7;
}
headerbar {
  background: #111318;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: none;
}
.hero-title { font-size: 30px; font-weight: 800; }
.hero-copy { color: #9da5b4; font-size: 14px; }
.section-title { font-size: 13px; font-weight: 700; color: #c8cdd7; }
.capture-card {
  background: #1b1e25;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 14px;
  padding: 16px;
}
.capture-card:hover {
  background: #242833;
  border-color: #7c6cff;
}
.capture-card:active { background: #2c3040; }
.capture-icon {
  background: #6d5dfc;
  color: white;
  border-radius: 10px;
  padding: 10px;
}
.action-title { font-size: 16px; font-weight: 700; }
.action-copy { color: #9da5b4; font-size: 12px; }
.shortcut {
  background: rgba(255, 255, 255, 0.08);
  color: #c8cdd7;
  border-radius: 7px;
  padding: 5px 8px;
  font-size: 11px;
}
.shortcut-button {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.10);
  border-radius: 10px;
  padding: 10px;
}
.status { color: #9da5b4; font-size: 12px; }
"#;

#[derive(Debug)]
pub enum UiMessage {
    ShowWindow,
    Quit,
    Status(String),
    Shortcuts(Vec<(String, String)>),
}

pub fn run() -> Result<()> {
    let application = Application::builder().application_id(APP_ID).build();
    let (action_sender, action_receiver) = unbounded_channel();
    let (ui_sender, ui_receiver) = std::sync::mpsc::channel();
    tray::start_worker(action_sender.clone(), action_receiver, ui_sender)?;

    let receiver = Rc::new(RefCell::new(Some(ui_receiver)));
    application.connect_activate(move |application| {
        if let Some(window) = application.active_window() {
            window.present();
            return;
        }
        let Some(ui_receiver) = receiver.borrow_mut().take() else {
            return;
        };
        build_window(application, action_sender.clone(), ui_receiver);
    });
    application.run();
    Ok(())
}

fn build_window(
    application: &Application,
    sender: tokio::sync::mpsc::UnboundedSender<AppAction>,
    receiver: std::sync::mpsc::Receiver<UiMessage>,
) {
    install_css();

    let window = ApplicationWindow::builder()
        .application(application)
        .title("Blink")
        .default_width(520)
        .default_height(590)
        .resizable(false)
        .build();
    window.connect_close_request(|window| {
        window.hide();
        glib::Propagation::Stop
    });

    let header = HeaderBar::new();
    header.set_show_title_buttons(true);
    let brand = Label::new(Some("BLINK"));
    brand.add_css_class("heading");
    header.set_title_widget(Some(&brand));
    let menu_button = MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Blink menu")
        .build();
    header.pack_end(&menu_button);
    window.set_titlebar(Some(&header));

    install_actions(application, &sender);
    let menu = gio::Menu::new();
    menu.append(Some("Open Screenshots Folder"), Some("app.open-folder"));
    menu.append(Some("Configure Shortcuts"), Some("app.shortcuts"));
    menu.append(Some("Quit Blink"), Some("app.quit"));
    menu_button.set_menu_model(Some(&menu));

    let content = GtkBox::new(Orientation::Vertical, 12);
    content.set_margin_top(28);
    content.set_margin_bottom(24);
    content.set_margin_start(28);
    content.set_margin_end(28);

    let title = Label::new(Some("Capture anything."));
    title.set_halign(Align::Start);
    title.add_css_class("hero-title");
    content.append(&title);

    let copy = Label::new(Some("Fast screenshots, without getting in your way."));
    copy.set_halign(Align::Start);
    copy.add_css_class("hero-copy");
    content.append(&copy);

    let section = Label::new(Some("CAPTURE"));
    section.set_halign(Align::Start);
    section.set_margin_top(16);
    section.add_css_class("section-title");
    content.append(&section);

    let shortcut_labels = Rc::new(RefCell::new(HashMap::new()));
    for (id, title, description, icon, shortcut, mode) in [
        (
            "capture-area",
            "Capture Area",
            "Draw a rectangle around what you need",
            "selection-mode-symbolic",
            "Print",
            CaptureMode::Area,
        ),
        (
            "capture-screen",
            "Capture Screen",
            "Capture the current display",
            "video-display-symbolic",
            "Shift + Print",
            CaptureMode::Screen,
        ),
        (
            "capture-window",
            "Capture Window",
            "Choose a single application window",
            "focus-windows-symbolic",
            "Alt + Print",
            CaptureMode::Window,
        ),
    ] {
        let (button, shortcut_label) = capture_button(
            title,
            description,
            icon,
            shortcut,
            mode,
            sender.clone(),
            window.clone(),
        );
        shortcut_labels
            .borrow_mut()
            .insert(id.to_owned(), shortcut_label);
        content.append(&button);
    }

    let configure = Button::with_label("Configure global shortcuts");
    configure.set_margin_top(8);
    configure.add_css_class("shortcut-button");
    let shortcut_sender = sender.clone();
    configure.connect_clicked(move |_| queue(&shortcut_sender, AppAction::ConfigureShortcuts));
    content.append(&configure);

    let status = Label::new(Some("Screenshots are saved to ~/Pictures/Blink"));
    status.set_margin_top(4);
    status.add_css_class("status");
    content.append(&status);

    window.set_child(Some(&content));
    window.present();

    listen_for_worker_messages(
        application.clone(),
        window,
        status,
        shortcut_labels,
        receiver,
    );
}

fn capture_button(
    title: &str,
    description: &str,
    icon_name: &str,
    shortcut: &str,
    mode: CaptureMode,
    sender: tokio::sync::mpsc::UnboundedSender<AppAction>,
    window: ApplicationWindow,
) -> (Button, Label) {
    let button = Button::new();
    button.add_css_class("capture-card");
    let row = GtkBox::new(Orientation::Horizontal, 14);

    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(24);
    icon.add_css_class("capture-icon");
    row.append(&icon);

    let text = GtkBox::new(Orientation::Vertical, 3);
    text.set_hexpand(true);
    let title_label = Label::new(Some(title));
    title_label.set_halign(Align::Start);
    title_label.add_css_class("action-title");
    text.append(&title_label);
    let copy = Label::new(Some(description));
    copy.set_halign(Align::Start);
    copy.add_css_class("action-copy");
    text.append(&copy);
    row.append(&text);

    let shortcut_label = Label::new(Some(shortcut));
    shortcut_label.add_css_class("shortcut");
    row.append(&shortcut_label);
    button.set_child(Some(&row));

    button.connect_clicked(move |_| {
        window.hide();
        queue(&sender, AppAction::Capture(mode));
    });
    (button, shortcut_label)
}

fn install_actions(
    application: &Application,
    sender: &tokio::sync::mpsc::UnboundedSender<AppAction>,
) {
    for (name, action) in [
        ("open-folder", AppAction::OpenFolder),
        ("shortcuts", AppAction::ConfigureShortcuts),
        ("quit", AppAction::Quit),
    ] {
        let simple_action = gio::SimpleAction::new(name, None);
        let sender = sender.clone();
        simple_action.connect_activate(move |_, _| queue(&sender, action));
        application.add_action(&simple_action);
    }
}

fn listen_for_worker_messages(
    application: Application,
    window: ApplicationWindow,
    status: Label,
    shortcut_labels: Rc<RefCell<HashMap<String, Label>>>,
    receiver: std::sync::mpsc::Receiver<UiMessage>,
) {
    glib::timeout_add_local(Duration::from_millis(80), move || {
        while let Ok(message) = receiver.try_recv() {
            match message {
                UiMessage::ShowWindow => window.present(),
                UiMessage::Quit => {
                    application.quit();
                    return glib::ControlFlow::Break;
                }
                UiMessage::Status(message) => status.set_text(&message),
                UiMessage::Shortcuts(shortcuts) => {
                    for (id, trigger) in shortcuts {
                        if let Some(label) = shortcut_labels.borrow().get(&id) {
                            label.set_text(&trigger);
                        }
                    }
                    status.set_text("Global shortcuts are active");
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

fn queue(sender: &tokio::sync::mpsc::UnboundedSender<AppAction>, action: AppAction) {
    if let Err(error) = sender.send(action) {
        eprintln!("Blink: action could not be queued: {error}");
    }
}

fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = CssProvider::new();
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
