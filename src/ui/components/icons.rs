use gpui::*;

pub struct IconView;

impl IconView {
    pub fn build<T: std::fmt::Debug>(icon: T, size: Pixels) -> Div {
        div()
            .w(size)
            .h(size)
            .bg(rgb(0x3C3C3C))
            .rounded(Pixels::from(2.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(rgb(0xFFFFFF))
                    .text_size(AbsoluteLength::Pixels(Pixels::from(8.0)))
                    .child(format!("{:?}", icon)),
            )
    }
}
