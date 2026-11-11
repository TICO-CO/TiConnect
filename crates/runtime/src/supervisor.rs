use std::future::Future;

use tokio::task::JoinSet;

use crate::{Error, Result};

#[derive(Default)]
pub struct Supervisor {
    tasks: JoinSet<Result<()>>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn<F>(&mut self, name: &'static str, future: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        let _ = name;
        self.tasks.spawn(future);
    }

    pub async fn join_next(&mut self) -> Option<Result<()>> {
        match self.tasks.join_next().await {
            Some(Ok(result)) => Some(result),
            Some(Err(error)) => Some(Err(Error::Runtime(format!("task failed: {error}")))),
            None => None,
        }
    }

    pub fn abort_all(&mut self) {
        self.tasks.abort_all();
    }
}
