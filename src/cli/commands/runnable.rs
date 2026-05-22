#[cfg(feature = "online")]
use tokio::runtime::Runtime;

pub type CommandResult = anyhow::Result<()>;

pub trait Runnable {
    fn run(&self) -> CommandResult;
}

#[cfg(feature = "online")]
pub trait AsyncRunnable: Runnable {
    async fn run_async(&self) -> CommandResult
    where
        Self: Sized;
}

#[cfg(feature = "online")]
impl<T> Runnable for T
where
    T: AsyncRunnable,
{
    fn run(&self) -> CommandResult {
        // Create a runtime to execute the asynchronous task
        let rt = Runtime::new()?;

        // Execute the asynchronous task
        rt.block_on(self.run_async())
    }
}
