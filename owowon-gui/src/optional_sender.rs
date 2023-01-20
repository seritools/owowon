use tokio::sync::mpsc::{self, error::SendError};

pub struct OptionalSender<T>(pub Option<mpsc::Sender<T>>);

impl<T> OptionalSender<T> {
    #[track_caller]
    pub fn blocking_send(&self, value: impl Into<T>) -> Result<(), SendError<T>> {
        if let Some(sender) = &self.0 {
            sender.blocking_send(value.into())
        } else {
            Ok(())
        }
    }
}
