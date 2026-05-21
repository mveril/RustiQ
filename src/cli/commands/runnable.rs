use tokio::runtime::Runtime;

pub trait Runnable {
    fn run(&self);
}

pub trait AsyncRunnable: Runnable {
    async fn run_async(&self);
}

impl<T> Runnable for T
where
    T: AsyncRunnable,
{
    fn run(&self) {
        // Crée un runtime pour exécuter la tâche asynchrone
        let rt = Runtime::new().expect("Failed to create runtime");

        // Exécute la tâche asynchrone
        rt.block_on(self.run_async());
    }
}
