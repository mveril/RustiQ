use tokio::runtime::Runtime;

pub trait Runable {
    fn run(&self);
}

pub trait AsyncRunable: Runable {
    async fn run_async(&self);
}

impl<T> Runable for T
where
    T: AsyncRunable,
{
    fn run(&self) {
        // Crée un runtime pour exécuter la tâche asynchrone
        let rt = Runtime::new().expect("Failed to create runtime");

        // Exécute la tâche asynchrone
        rt.block_on(self.run_async());
    }
}
