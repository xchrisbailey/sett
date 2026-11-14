# gpui-kit 0.9.1 components for Labels

Research for [#102](https://github.com/xchrisbailey/sett/issues/102), part of the map [#101 Labels on Tasks](https://github.com/xchrisbailey/sett/issues/101). Checked against the crate source in the cargo registry, not the published docs. Nothing here was compiled or prototyped.

`C` = `~/.cargo/registry/src/index.crates.io-*/gpui-component-0.9.1/src`. Every module below is public in `C/lib.rs` (`badge` :33, `combobox` :43, `popover` :69, `tag` :92) and reached as `gpui_kit::component::<module>`.

## Summary

| Need | Use | Gap | Cheapest fill |
|---|---|---|---|
| Label chip on a Card | `tag::Tag` | No truncation, no remove button | `.max_w(..)` on the Tag, with a `div().truncate()` child |
| Task view Label field | `combobox::Combobox` with `.multiple(true).searchable(true)` | Can't create a value from typed text. The default trigger joins titles with ", " | Our own `SearchableListDelegate` that adds an "Add “…”" row, plus `render_trigger` drawing Tags with an x button |
| Board Label filter | `combobox::Combobox` with `.multiple(true)` | None worth noting | A `render_trigger` that shows "Labels" and a count |
| Palette | `theme::ColorName` via `Tag::color(..)` | None | Hash the lowercased Label onto a fixed list of `ColorName`s |

Don't use `Badge` for Labels. It only draws a count, a dot or an icon on top of another element (`C/badge.rs:8-14`).

## 1. A chip for one Label on a Card: `Tag`

`C/tag.rs`. `Tag::new()` (:135), plus shorthands `primary/secondary/danger/success/warning/info` (:147-174), `Tag::color(impl Into<ColorName>)` (:186) and `Tag::custom(bg, fg, border)` (:177). It also has `.outline()` (:197), `.rounded(..)` / `.rounded_full()` (:203-212), `Sizable` (:215), `ParentElement` (:222) and `Styled` (:228).

- **Sizes** (:256-264): `Small`/`XSmall` gives 0.625rem text, `px_1p5 py_0p5` and radius `theme.radius / 2`. The default `Medium` gives `text_xs` with `px_2p5 py_1`. `Large` gives `text_sm`. The doc comment says only Medium and Small are supported (:122). On a Card, `.small()` sits well next to the existing `xsmall` Due and checklist icons in `board.rs` `render_card`.
- **Colour variants:** `TagVariant::Color(ColorName)` (:18) picks shades by theme mode. Light uses bg `scale(50)`, border `scale(200)`, fg `scale(600)`. Dark uses bg `scale(950)` at 0.5 opacity, border `scale(800)` at 0.5, fg `scale(300)` (:35-41, :54-60, :109-115). So one `ColorName` gives a readable chip in both modes without any theme lookup.
- **Truncation: none.** The Tag is a flex `div` that renders its children (:251-271), with no `max_w`, `overflow_hidden` or `truncate`. Fill: `Tag::color(c).small().max_w(px(120.)).child(div().truncate().child(label))`, and let the Card's chip row `flex_wrap()` so many Labels wrap rather than overflow. Set the `max_w` in the prototype.
- **No remove button.** Not needed on a Card, since the map says Cards don't edit Labels. For the Task view, add `Button::new(id).ghost().xsmall().icon(IconName::X)` as a second child (see 2).
- **Hover:** every Tag dims to 0.9 opacity on hover (:269), even when nothing is clickable. It's harmless, and `.refine_style` comes after it, so a style override can undo it if it reads as clickable.

## 2. The Task view field: `Combobox` (multi, searchable) plus a custom delegate

`C/combobox.rs`. It's the closest thing gpui-kit has to a tag input. There is no tag-input component. `Select` (`C/select.rs`) is single-value only and has no `multiple`.

**What exists:**
- `ComboboxState::new(delegate: D, selected: Vec<IndexPath>, window, cx)` (:145), where `D: SearchableListDelegate`. Then `.multiple(true)` (:295) keeps the popup open and toggles items on click, and `.searchable(true)` (:301) adds a search input at the top of the popup.
- Reads and writes: `selected_values()`, `selection()`, `set_selected_values`, `add_selected_index`, `remove_selected_index`, `clear_selection`, `set_items`, `query` and `set_query` (:307-415).
- Events: `ComboboxEvent::Change(Vec<Value>)` on every toggle and `ComboboxEvent::Confirm(Vec<Value>)` when the popup closes (:129-137).
- Element: `Combobox::new(&state)` (:787) with `.placeholder`, `.search_placeholder`, `.cleanable`, `.disabled`, `.menu_width`, `.menu_max_h`, `.empty(..)`, `.appearance`, `.footer(..)` and `.render_trigger(|ctx, window, cx| ..)` (:799-882). It also implements `Sizable` and `Styled`. `ComboboxTriggerContext` exposes `selection()`, `placeholder()`, `is_open()`, `is_disabled()` and `size()` (:42-63).
- Nested buttons in a custom trigger do get clicks. `dismiss` deliberately lets presses on the trigger propagate "so a tag remove button … can see the press" (:534-541).
- Used in the crate's own tests: `gpui-kit-0.9.1/tests/integration/search.rs:199-260`.

**What it lacks:**
1. **Creating a value from typed text.** Search only filters the delegate's items (`SearchableVec::perform_search`, `C/searchable_list/vec.rs:167-177`), and there's no "allow create" option.
2. **Chips in the trigger.** In multi mode the default trigger draws the selected titles joined with ", " and truncates them (:580-593).
3. Single-line `Input` has no suggestion popup. Completion menus belong to the code-editor input mode only (`C/input/overlay.rs:106-118`, :169).

**Cheapest fill (recommended):** write a small `LabelOptions` delegate that implements `SearchableListDelegate` (`C/searchable_list/delegate.rs:54`):
- `items`: the Space's Labels (derived from loaded Tasks), as `SharedString`.
- `perform_search(query)` (:86): filter case-insensitively (the default `SearchableListItem::matches` is already a case-insensitive substring match, :43). When the trimmed query matches no Label ignoring case, append a synthetic item for the query, rendered through `render_item` (:102) as "Add “query”". After every search the list highlights the first row (`C/list/list.rs:308`), so Enter takes the best match. Put the Add row first only when nothing matches.
- `on_will_change(selection, changes)` (:167) may edit the selection freely, so case-insensitive dedupe with "first spelling wins" can live here. It has no `cx`, so the write to frontmatter happens in the view's `ComboboxEvent::Change` subscription, the same way `task_view.rs` handles `SelectEvent` for Column.
- `render_trigger`: `h_flex().flex_wrap().gap_1()` of `Tag`s from `ctx.selection()`. Each Tag gets an x `Button` that calls `remove_selected_index`, and the placeholder shows when there are none. Size the field to match the Column `Select` and Due `DatePicker` rows (`.small()`).

Things the prototype has to confirm: whether a selected synthetic item survives a later `set_items` (rebuild the items, with the new Label included, on every Change), what Enter does on an empty query, and how the trigger grows in height as chips wrap.

**Alternatives considered and rejected:**
- An `Input` plus our own suggestion list in a `Popover`. `Popover::trigger` needs a `Selectable` element such as a Button, not an Input (`C/popover.rs:189-200`), and the anchored `dropdown_popup` helper that Combobox uses is `pub(crate)` (`C/popover.rs:84`). We'd be rebuilding positioning, keyboard navigation and dismissal by hand.
- An `Input` plus an inline list under it, with no popover. It's the cheapest code but pushes the editor down and has no keyboard list navigation. Keep it as the fallback if the Combobox delegate fights back.

## 3. Board header filter: `Combobox` (multi)

Same component: `ComboboxState::new(SearchableVec::new(labels), vec![], window, cx).multiple(true)`. Add `.searchable(true)` only if a Space has many Labels. Then `Combobox::new(&state).small().cleanable(true).placeholder("Labels")`, with a `render_trigger` showing "Labels" or "Labels · 2" so the header stays one line. Subscribe to `ComboboxEvent::Change` to filter live. The state is a field on the Board view and is never saved, as the map wants. Refresh the options with `set_items` when Tasks reload.

`DropdownMenu` with `PopupMenuItem::checked(bool)` (`C/menu/popup_menu.rs:186`) also renders check marks, and `board.rs` already uses `.dropdown_menu` for Column actions. But every item click dismisses the menu (`activate` calls `dismiss`, `C/menu/popup_menu.rs:891-919`), so picking two Labels means opening it twice. Combobox stays open in multi mode (:291-293). Use Combobox.

## 4. A fixed Label palette

sett sets no theme of its own, so it runs on gpui-kit's default light and dark themes.

**Recommended:** `ColorName` (`C/theme/color.rs:375`), the Tailwind palette. `ColorName::all()` returns 19 names (:445), and `scale(n)` takes 50–950 (:473). Through `Tag::color`, each name gets its light and dark shades for free (section 1). Choose 8 hues that are easy to tell apart and none of which is `theme.danger` red, which `render_card` already uses for overdue:

`Orange, Amber, Lime, Emerald, Cyan, Blue, Violet, Pink`

Pick each Label's colour as a stable hash (FNV or a byte sum, not `DefaultHasher`, which isn't guaranteed stable across releases) of its lowercased text, modulo 8. That way, Labels equal ignoring case get the same colour on every Card and in every Space, and there's nothing to store.

**Theme fields, if colours should follow a custom theme later:** `ThemeColor` has `red/green/blue/yellow/magenta/cyan` and each one's `_light` variant (`C/theme/theme_color.rs:322-344`), plus `chart_1`–`chart_5` (:135-143). These are single values per theme, not bg/fg/border triples. You'd go through `Tag::custom(bg, fg, border)` and pick the opacities by hand, and with only six hues collisions come sooner. Not worth it now.
