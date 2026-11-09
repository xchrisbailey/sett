use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

actions!(sett, [Quit]);

struct Shell;

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .child("sett")
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);

        // Bind before set_menus: menus copy shortcuts from the keymap at call time.
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus(vec![
            Menu::new("sett").items([MenuItem::action("Quit sett", Quit)]),
        ]);

        let bounds = Bounds::centered(None, size(px(1024.), px(720.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("sett".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        open_window(options, cx, |_, cx| cx.new(|_| Shell)).expect("failed to open window");
        cx.activate(true);
    });
}
