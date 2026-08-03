use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, DrawingArea,
    EventControllerKey, GestureDrag, HeaderBar, Image, Label, MenuButton, Orientation, Overlay,
    Picture, Switch, gio, glib, prelude::*,
};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    app::AppAction,
    capture::{CaptureMode, NormalizedRegion},
    error::Result,
    settings,
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
.setting-row {
  background: #1b1e25;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  padding: 12px;
}
.status { color: #9da5b4; font-size: 12px; }
"#;

#[derive(Debug)]
pub enum UiMessage {
    ShowWindow,
    HideWindow,
    Quit,
    Status(String),
    Shortcuts(Vec<(String, String)>),
    BeginAreaSelection(std::path::PathBuf),
    CopyToClipboard(std::path::PathBuf),
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

    let clipboard_row = GtkBox::new(Orientation::Horizontal, 12);
    clipboard_row.add_css_class("setting-row");
    let clipboard_text = GtkBox::new(Orientation::Vertical, 2);
    clipboard_text.set_hexpand(true);
    let clipboard_title = Label::new(Some("Copy captures to clipboard"));
    clipboard_title.set_halign(Align::Start);
    clipboard_title.add_css_class("action-title");
    clipboard_text.append(&clipboard_title);
    let clipboard_copy = Label::new(Some("Also copy each saved screenshot automatically"));
    clipboard_copy.set_halign(Align::Start);
    clipboard_copy.add_css_class("action-copy");
    clipboard_text.append(&clipboard_copy);
    clipboard_row.append(&clipboard_text);
    let clipboard_switch = Switch::new();
    clipboard_switch.set_valign(Align::Center);
    clipboard_switch.set_active(settings::Settings::load().copy_to_clipboard);
    let clipboard_sender = sender.clone();
    clipboard_switch.connect_active_notify(move |switch| {
        queue(
            &clipboard_sender,
            AppAction::SetCopyToClipboard(switch.is_active()),
        );
    });
    clipboard_row.append(&clipboard_switch);
    content.append(&clipboard_row);

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
        sender,
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
        simple_action.connect_activate(move |_, _| queue(&sender, action.clone()));
        application.add_action(&simple_action);
    }
}

fn listen_for_worker_messages(
    application: Application,
    window: ApplicationWindow,
    status: Label,
    shortcut_labels: Rc<RefCell<HashMap<String, Label>>>,
    receiver: std::sync::mpsc::Receiver<UiMessage>,
    sender: tokio::sync::mpsc::UnboundedSender<AppAction>,
) {
    glib::timeout_add_local(Duration::from_millis(80), move || {
        while let Ok(message) = receiver.try_recv() {
            match message {
                UiMessage::ShowWindow => window.present(),
                UiMessage::HideWindow => window.hide(),
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
                UiMessage::BeginAreaSelection(source) => {
                    show_area_selector(&application, &window, source, sender.clone());
                }
                UiMessage::CopyToClipboard(path) => match gtk::gdk::Texture::from_filename(&path) {
                    Ok(texture) => {
                        if let Some(display) = gtk::gdk::Display::default() {
                            display.clipboard().set_texture(&texture);
                            status.set_text("Screenshot saved and copied to clipboard");
                        } else {
                            status.set_text("Screenshot saved, but the clipboard is unavailable");
                        }
                    }
                    Err(error) => status.set_text(&format!(
                        "Screenshot saved, but clipboard copy failed: {error}"
                    )),
                },
            }
        }
        glib::ControlFlow::Continue
    });
}

#[derive(Default)]
struct Selection {
    start_x: f64,
    start_y: f64,
    current_x: f64,
    current_y: f64,
    active: bool,
}

fn show_area_selector(
    application: &Application,
    main_window: &ApplicationWindow,
    source: std::path::PathBuf,
    sender: tokio::sync::mpsc::UnboundedSender<AppAction>,
) {
    let selector = ApplicationWindow::builder()
        .application(application)
        .title("Select an area")
        .decorated(false)
        .build();
    selector.set_cursor_from_name(Some("crosshair"));

    let overlay = Overlay::new();
    let picture = Picture::for_filename(&source);
    picture.set_content_fit(gtk::ContentFit::Fill);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    overlay.set_child(Some(&picture));

    let drawing = DrawingArea::new();
    drawing.set_hexpand(true);
    drawing.set_vexpand(true);
    let selection = Rc::new(RefCell::new(Selection::default()));
    let draw_selection = selection.clone();
    drawing.set_draw_func(move |_, context, width, height| {
        context.set_source_rgba(0.02, 0.03, 0.05, 0.58);
        context.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
        let _ = context.fill();

        let selection = draw_selection.borrow();
        if selection.active {
            let x = selection.start_x.min(selection.current_x);
            let y = selection.start_y.min(selection.current_y);
            let width = (selection.current_x - selection.start_x).abs();
            let height = (selection.current_y - selection.start_y).abs();
            context.set_operator(gtk::cairo::Operator::Clear);
            context.rectangle(x, y, width, height);
            let _ = context.fill();
            context.set_operator(gtk::cairo::Operator::Over);
            context.set_source_rgb(0.43, 0.36, 0.99);
            context.set_line_width(2.0);
            context.rectangle(
                x + 1.0,
                y + 1.0,
                (width - 2.0).max(0.0),
                (height - 2.0).max(0.0),
            );
            let _ = context.stroke();
        }
    });

    let drag = GestureDrag::new();
    let begin_selection = selection.clone();
    let begin_drawing = drawing.clone();
    drag.connect_drag_begin(move |_, x, y| {
        let mut selection = begin_selection.borrow_mut();
        selection.start_x = x;
        selection.start_y = y;
        selection.current_x = x;
        selection.current_y = y;
        selection.active = true;
        begin_drawing.queue_draw();
    });
    let update_selection = selection.clone();
    let update_drawing = drawing.clone();
    drag.connect_drag_update(move |_, offset_x, offset_y| {
        let mut selection = update_selection.borrow_mut();
        selection.current_x = selection.start_x + offset_x;
        selection.current_y = selection.start_y + offset_y;
        update_drawing.queue_draw();
    });
    let end_selection = selection.clone();
    let end_drawing = drawing.clone();
    let end_selector = selector.clone();
    let end_main_window = main_window.clone();
    let end_source = source.clone();
    drag.connect_drag_end(move |_, offset_x, offset_y| {
        let selection = end_selection.borrow();
        let end_x = selection.start_x + offset_x;
        let end_y = selection.start_y + offset_y;
        let x = selection.start_x.min(end_x);
        let y = selection.start_y.min(end_y);
        let width = (end_x - selection.start_x).abs();
        let height = (end_y - selection.start_y).abs();
        let view_width = f64::from(end_drawing.width()).max(1.0);
        let view_height = f64::from(end_drawing.height()).max(1.0);
        drop(selection);

        if width >= 3.0 && height >= 3.0 {
            queue(
                &sender,
                AppAction::FinishAreaCapture {
                    source: end_source.clone(),
                    region: NormalizedRegion {
                        x: x / view_width,
                        y: y / view_height,
                        width: width / view_width,
                        height: height / view_height,
                    },
                },
            );
            end_selector.close();
        } else {
            end_selector.close();
            end_main_window.present();
        }
    });
    drawing.add_controller(drag);
    overlay.add_overlay(&drawing);

    let keys = EventControllerKey::new();
    let key_selector = selector.clone();
    let key_main_window = main_window.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            key_selector.close();
            key_main_window.present();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    selector.add_controller(keys);
    selector.set_child(Some(&overlay));
    selector.fullscreen();
    selector.present();
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
