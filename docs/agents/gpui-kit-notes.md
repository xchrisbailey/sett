# gpui-kit 0.9.1 notes

Checked against the crate source in `~/.cargo/registry/src/index.crates.io-*/` during v1. The snippets were not compiled. The published docs at gpui-kit.com lag 0.9.1; where they disagree with this file, trust the source. When you bump gpui-kit, check the signatures again and update this file.

`C` = `gpui-component-0.9.1/src`, `B` = `gpui-base-0.9.1/src`, `G` = `gpui-pre-0.3.12/src`, `A` = `gpui-kit-assets-0.9.1`, all under `~/.cargo/registry/src/index.crates.io-*/`.

Imports: `use gpui_kit::*;` gives GPUI. `gpui_kit::component` re-exports `h_flex`, `v_flex`, `ActiveTheme`, `Icon`, `IconName`, `TitleBar`, `WindowExt`, `Sizable`, `Side`, `IndexPath` and `Collapsible`.

## Project choices

- **Project icons:** a curated set in `crates/app/src/icons.rs`, embedded through `icon_assets!` and layered over `gpui_kit::assets::Assets`. Add icons there, not through `AllAssets`.
- **Dates:** `chrono` is used only in `crates/app`, for the DatePicker. Convert with `crates/app/src/dates.rs`. `sett-store` keeps its own `Date`.
- **Watcher events** reach GPUI over `async-channel` (`Watcher::events()`).

## Testing gotchas

- **A real file watcher breaks UI tests.** Its thread wakes GPUI's test executor, and the test fails. Build the model with `AppModel::open_unwatched` and feed changes in through `AppModel::apply_change`. The watcher itself is tested in `crates/store/tests/watcher.rs`.
- **Tests that trash set their own Trash** with `Store::set_trash` or `AppModel::set_trash`. Only `AppModel::open` turns on the real macOS Trash; without either, trashing fails.
- **Shared UI helpers** live in `crates/app/tests/common/mod.rs`: `open_shell`, `click_label`, `right_click_label`, `find_label`.
- **`test-support`** is enabled on gpui-kit only for `crates/app` (dev-dependency). Turning it on forced `taffy` down to 0.15 and `unicode-properties` down to 0.1.3 in the lockfile. The forked editor's own view tests sit behind its `view-tests` feature because `test-support` doesn't resolve there.
- **Looking at the UI:** `scripts/screenshot` captures the real window against fixture data. Read the PNG to check layout yourself.

## Doc and source mismatches
The `/component/*.md` doc pages disagree with the source in places:
- `Sidebar::new` needs an id.
- `SidebarMenuItem::suffix` takes a closure.
- `AlertDialog` has no `.warning()`.
- `SidebarGroup` can't fold.
- `Menu` gained a `disabled` field in 0.9.1, so a struct literal won't compile. Use `Menu::new(name).items([...])`.

## Bootstrap
`gpui_kit::open_window(options, cx, |w, cx| cx.new(..))` already wraps the view in `base::Root` (`gpui-kit/src/lib.rs:181`). Don't add another `Root`.

## TitleBar and the Space toggle (#5)
- `C/title_bar.rs`: `TITLE_BAR_HEIGHT = px(34.)`. On macOS the left padding is a fixed `px(80.)` for the traffic lights. Children go in an `h_flex` that acts as the drag area.
- Window options must start from `TitleBar::window_options()`.
- For the toggle, use `TabBar::segmented()`, which always keeps exactly one tab selected. Don't use `ToggleGroup::segmented().single()` (clicking the checked item clears it) or `ButtonGroup` (no selected state of its own).
- Wrap the toggle in a div that calls `cx.stop_propagation()` on mouse-down; otherwise the TitleBar treats the press as a window drag. This is inferred from source, not tried in a window.

```rust
use gpui_kit::component::{TitleBar, h_flex, v_flex, Sizable as _, tab::{Tab, TabBar}};

let options = WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(bounds)),
    ..TitleBar::window_options()
};

// in render
v_flex().size_full()
    .child(TitleBar::new().child(
        h_flex().flex_1().justify_center().child(
            div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(
                TabBar::new("space-toggle").segmented().small()
                    .selected_index(self.space_index)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| { this.space_index = *ix; cx.notify(); }))
                    .child(Tab::new().label("Personal"))
                    .child(Tab::new().label("Work")),
            ),
        ),
    ))
    .child(/* body */)
```

## Sidebar (#5)
- `Sidebar<E: SidebarItem>::new(id)` has `.header(..)`, `.footer(..)`, `.child(E)`, `.children(..)`, `.collapsible(..)`, `.collapsed(bool)` and `.side(Side)`. Default width is 255px.
- **Every child has the same type `E`.** Use `Sidebar<SidebarMenu>` with several menus, or `Sidebar<SidebarGroup<SidebarMenu>>`. You can't mix them.
- `SidebarGroup::new(label)`: its `collapsed` means icon-only mode, not folding.
- `SidebarMenuItem::new(label)` has:
  - `.icon(impl Into<Icon>)`, `.active(bool)`
  - `.on_click(Fn(&ClickEvent,&mut Window,&mut App))`
  - `.children([...])`, which makes it a submenu with a caret
  - `.default_open(bool)` (read on first render only), `.click_to_toggle(bool)`
  - `.suffix(|window, cx| el)`, `.context_menu(|menu, w, cx| menu)`, `.disable(bool)`

```rust
use gpui_kit::component::{IconName, sidebar::{Sidebar, SidebarHeader, SidebarMenu, SidebarMenuItem}};

Sidebar::new("nav").collapsible(false)
    .header(SidebarHeader::new().child("Personal"))
    .child(SidebarMenu::new()
        .child(SidebarMenuItem::new("Journal").icon(IconName::BookOpen).active(sel == Journal)
            .on_click(cx.listener(|this, _, _, cx| this.select(Journal, cx))))
        .child(SidebarMenuItem::new("Notes").icon(IconName::NotebookPen)))
    .child(SidebarMenu::new().children(projects.iter().map(|p|
        SidebarMenuItem::new(p.name.clone()).icon(p.icon))))
    .child(SidebarMenu::new().child(
        SidebarMenuItem::new("Archived").icon(IconName::Archive)
            .default_open(false).click_to_toggle(true)
            .children(archived.iter().map(|p| SidebarMenuItem::new(p.name.clone())))))
```
Verify icon variant names first: kebab-case filenames become PascalCase (`folder-open` becomes `FolderOpen`).

## Center panel and breadcrumb
- There's no router. Keep a selection enum in the shell and `match` on it in render, holding child view entities as fields.
- `C/breadcrumb.rs`: `Breadcrumb::new().child(BreadcrumbItem::new(label).on_click(..).disabled(bool))`.

## Drag and drop (kanban, #7, `G/elements/div.rs`)
- `on_drag<T, W: Render>(value, Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<W>)` is on `StatefulInteractiveElement`, so the element needs `.id(..)`.
- `drag_over::<T>`, `group_drag_over::<T>`, `on_drop::<T>(Fn(&T,&mut Window,&mut App))`, `on_drag_move::<T>` (`DragMoveEvent { event, bounds }`, `.drag(cx)`) and `can_drop` are on `InteractiveElement`.
- The theme provides `cx.theme().drag_border`. The pattern follows `C/dock/tab_panel.rs:597-630`.
- Drops bubble, so call `cx.stop_propagation()` in the card's `on_drop`.

```rust
#[derive(Clone)] struct DraggedCard { task_id: TaskId, title: SharedString }
struct CardPreview { title: SharedString }
impl Render for CardPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().px_3().py_2().w(px(240.)).rounded(cx.theme().radius).border_1()
            .border_color(cx.theme().border).bg(cx.theme().background).shadow_md().child(self.title.clone())
    }
}
div().id(ElementId::Name(format!("card-{}", t.id).into()))
    .cursor_grab()
    .on_drag(DraggedCard { task_id: t.id.clone(), title: t.title.clone() },
             |d, _offset, _, cx| cx.new(|_| CardPreview { title: d.title.clone() }))
    .drag_over::<DraggedCard>(|s, _, _, cx| s.border_t_2().border_color(cx.theme().drag_border))
    .on_drop(cx.listener(move |this, d: &DraggedCard, _, cx| this.move_card(&d.task_id, col_id, Some(before_id), cx)))

v_flex().id(col_key).flex_1()
    .drag_over::<DraggedCard>(|s, _, _, cx| s.bg(cx.theme().muted))
    .on_drop(cx.listener(move |this, d: &DraggedCard, _, cx| this.move_card(&d.task_id, col_id, None, cx)))
```

## Horizontal row of vertical lists
- GPUI: `overflow_x_scroll()`, `overflow_y_scroll()`, `track_scroll(&ScrollHandle)` on stateful divs.
- gpui-kit: `gpui_kit::component::scroll::ScrollableElement as _` adds `.overflow_x_scrollbar()`, `.overflow_y_scrollbar()` and `.overflow_scrollbar()`. Call them last.

```rust
h_flex().id("board-columns").size_full().gap_3().p_3().items_start()
    .children(columns.iter().map(|c|
        v_flex().id(ElementId::Name(format!("col-{}", c.id).into()))
            .w(px(280.)).flex_shrink_0().h_full()
            .child(/* header + quick-add Input */)
            .child(v_flex().id(ElementId::Name(format!("col-body-{}", c.id).into()))
                .flex_1().gap_2().overflow_y_scrollbar())))
    .overflow_x_scrollbar()
```

## AlertDialog (#6 delete)
- `window.open_alert_dialog(cx, Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog)`. The builder may run again on rebuilds, so keep it cheap and idempotent.
- Methods: `.title`, `.description`, `.icon`, `.confirm()`, `.ok_text`, `.ok_variant(ButtonVariant::Danger)`, `.cancel_text`, `.on_ok(..)->bool` (true closes), `.on_cancel`, `.on_close`, `.width`.

```rust
use gpui_kit::component::{WindowExt as _, button::ButtonVariant};
let entity = cx.entity().downgrade();
window.open_alert_dialog(cx, move |alert, _, _| {
    let entity = entity.clone();
    alert.confirm().title("Delete project?")
        .description("The folder will be moved to the Trash.")
        .ok_text("Delete").ok_variant(ButtonVariant::Danger)
        .on_ok(move |_, _, cx| { entity.update(cx, |this, cx| this.delete_project(cx)).ok(); true })
});
```

## Dialog with a form (#6 create)
- A plain `Dialog` draws no buttons; supply `.footer(DialogFooter::new()...)`.
- Create `InputState` before `open_dialog`, and focus it on the first build only (a `Cell<bool>` guard).
- Close with `window.close_dialog(cx)`.
- `Form::new().child(Field::new().label(..).child(..))`.

```rust
use std::cell::Cell;
use gpui_kit::component::{WindowExt as _, button::{Button, ButtonVariants as _}, dialog::DialogFooter,
    form::{Form, Field}, input::{Input, InputState}};
let name = cx.new(|cx| InputState::new(window, cx).placeholder("Project name"));
let first = Cell::new(true);
let view = cx.entity().downgrade();
window.open_dialog(cx, move |dialog, window, cx| {
    if first.replace(false) { name.focus_handle(cx).focus(window, cx); }
    let (name2, view2) = (name.clone(), view.clone());
    dialog.title("New project")
        .child(Form::new()
            .child(Field::new().label("Name").child(Input::new(&name)))
            .child(Field::new().label("Icon").child(/* icon picker */)))
        .footer(DialogFooter::new()
            .child(Button::new("cancel").label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
            .child(Button::new("create").primary().label("Create").on_click(move |_, w, cx| {
                let n = name2.read(cx).value();
                view2.update(cx, |this, cx| this.create_project(n, cx)).ok();
                w.close_dialog(cx);
            })))
});
```

## Input
- `gpui_kit::component::input::{Input, InputState, InputEvent}`.
- State: `InputState::new(window, cx).placeholder(..).default_value(..)`, read with `.value()`, write with `.set_value(v, window, cx)` (emits no events).
- Events: `InputEvent::{Change, PressEnter { secondary, shift }, Focus, Blur}`. Subscribe in the constructor with `cx.subscribe_in(&input, window, ..)` and keep the `Subscription`.
- Element: `Input::new(&state).cleanable(true).prefix(..).suffix(..).appearance(bool).disabled(bool)`.

## DatePicker
- `gpui_kit::component::date_picker::{DatePicker, DatePickerState, DatePickerEvent}`, `calendar::Date`.
- State: `DatePickerState::new(window, cx).date_format("%Y-%m-%d")`, read with `.date() -> Date`, write with `.set_date(impl Into<Date>, window, cx)`.
- `Date::Single(Option<NaiveDate>)`. Events: `DatePickerEvent::Change(DateTime)`.
- Element: `DatePicker::new(&state).placeholder(..).cleanable(true)`.
- Needs `chrono` (see Open decisions).

## Select
- `gpui_kit::component::select::{Select, SelectState, SelectEvent, SelectItem, SearchableVec}`, `component::IndexPath`.
- State: `SelectState::new(Vec<T: SelectItem>, Option<IndexPath>, window, cx)`. `String`, `SharedString` and `&'static str` implement `SelectItem`.
- Events: `SelectEvent::Confirm(Option<Value>)`.
- Setters: `set_selected_index`, `set_selected_value`, `set_items`. Element: `Select::new(&state).placeholder(..)`.

## Icon picker
- `gpui_kit::assets::IconName` covers all 1,830 Lucide icons and is `Copy + Eq + Hash`.
- It has `IconName::ALL` and `.path() -> "icons/<kebab>.svg"`, but no `FromStr` or name accessor.
- Persist the kebab name and look it up with `ALL.iter().find(|i| i.path() == format!("icons/{name}.svg"))`.
- `component::IconName` is the default subset of about 106 icons (`A/default-icons.txt`), with no `ALL`.
- Rendering icons outside the default subset needs `.with_assets(gpui_kit::assets::AllAssets)` or your own `icon_assets!` source. sett uses the latter (see Project choices).
- `Icon::new(name)` works for any `IconNamed`.

## Actions and keybindings
- Use `gpui_kit::actions!`. `KeyBinding::new("cmd-1", Action, Option<&str context>)`.
- Bind before `set_menus`, because menus copy shortcuts at call time.
- Element-level: `.key_context("X").track_focus(&handle).on_action(cx.listener(..))`. Focus the shell on open with `handle.focus(window, cx)`.
- Components bind `escape` to `component::actions::Cancel` in their own contexts. The deeper context wins.

## Background work
- `cx.spawn(async move |this: WeakEntity<T>, cx: &mut AsyncApp| ..)`, `cx.background_spawn(fut)`.
- `this.update(cx, ..) -> Result`. `spawn_in(window, ..)` with `update_in` gives window access.
- A `Task` cancels when dropped: store it or `.detach()`. Timer: `cx.background_executor().timer(d)`.

```rust
// std::sync::mpsc::Receiver: hand it back each turn
let task = cx.spawn(async move |this, cx| {
    let mut rx = rx;
    loop {
        let (back, msg) = cx.background_spawn(async move { let m = rx.recv(); (rx, m) }).await;
        rx = back;
        let Ok(ev) = msg else { break };
        if this.update(cx, |s, cx| { s.on_fs_event(ev); cx.notify(); }).is_err() { break; }
    }
});
self._watch = task;
```
