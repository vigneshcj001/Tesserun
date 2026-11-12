use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};

/// One cancellable operation. Generation IDs reject results already queued
/// before cancellation, so an old task cannot overwrite a newer UI operation.
pub(super) struct TaskRunner<T> {
    runtime: tokio::runtime::Runtime,
    sender: Sender<(u64, Result<T, String>)>,
    receiver: Receiver<(u64, Result<T, String>)>,
    active: Option<tokio::task::JoinHandle<()>>,
    generation: u64,
}
impl<T: Send + 'static> TaskRunner<T> {
    pub(super) fn new() -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        Ok(Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?,
            sender,
            receiver,
            active: None,
            generation: 0,
        })
    }
    pub(super) fn spawn<F>(&mut self, ctx: egui::Context, work: F)
    where
        F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        self.cancel();
        let generation = self.generation;
        let sender = self.sender.clone();
        self.active = Some(self.runtime.spawn(async move {
            let result = work.await.map_err(|error| error.to_string());
            let _ = sender.send((generation, result));
            ctx.request_repaint();
        }));
    }
    pub(super) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(task) = self.active.take() {
            task.abort();
        }
    }
    pub(super) fn is_busy(&self) -> bool {
        self.active.is_some()
    }
    pub(super) fn poll(&mut self) -> Option<Result<T, String>> {
        // Sample completion before draining: a task sends its result before it
        // finishes, so a finished task with no queued result panicked.
        let finished = self
            .active
            .as_ref()
            .is_some_and(tokio::task::JoinHandle::is_finished);
        while let Ok((generation, result)) = self.receiver.try_recv() {
            if generation == self.generation && self.active.is_some() {
                self.active = None;
                return Some(result);
            }
        }
        if finished {
            self.active = None;
            return Some(Err(
                "The operation stopped unexpectedly. Please retry.".into()
            ));
        }
        None
    }
}
impl<T> Drop for TaskRunner<T> {
    fn drop(&mut self) {
        if let Some(task) = self.active.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_result_cannot_complete_a_new_operation() {
        let mut runner = TaskRunner::<u8>::new().unwrap();
        runner.spawn(egui::Context::default(), std::future::pending());
        let old = runner.generation;
        runner.cancel();
        runner.sender.send((old, Ok(1))).unwrap();
        assert!(runner.poll().is_none());
        runner.spawn(egui::Context::default(), std::future::pending());
        runner.sender.send((old, Ok(2))).unwrap();
        assert!(runner.poll().is_none());
        assert!(runner.is_busy());
        runner.sender.send((runner.generation, Ok(3))).unwrap();
        assert_eq!(runner.poll().unwrap().unwrap(), 3);
        assert!(!runner.is_busy());
    }
    #[test]
    fn panicked_task_reports_failure_instead_of_staying_busy() {
        let mut runner = TaskRunner::<u8>::new().unwrap();
        runner.spawn(egui::Context::default(), async {
            if std::hint::black_box(true) {
                panic!("fixture panic");
            }
            Ok(1)
        });
        while !runner.active.as_ref().unwrap().is_finished() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(runner.poll().unwrap().is_err());
        assert!(!runner.is_busy());
    }
}
