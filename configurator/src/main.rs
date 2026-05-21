mod app;
mod messages;
mod models;
#[cfg(test)]
mod test_temp;

fn main() -> iced::Result {
    app::run()
}
