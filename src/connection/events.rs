use std::time::Duration;

#[derive(Debug, Clone)]
pub enum StatEvent {
    Request { success: bool },
    ResponseTime(Duration),
}
