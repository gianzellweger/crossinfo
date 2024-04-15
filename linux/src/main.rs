use gtk::{prelude::*, *};
use gtk4 as gtk;

const APP_ID: &str = "org.crossinfo.crossinfo";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(ui);

    app.run()
}

fn ui(app: &Application) {
    let button = Button::builder().label("Press me!").margin_top(12).margin_bottom(12).margin_start(12).margin_end(12).build();

    button.connect_clicked(|button| {
        button.set_label("Hello World!");
    });
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(900)
        .default_height(600)
        .title("Crossinfo")
        .child(&button)
        .build();

    window.show();
}
