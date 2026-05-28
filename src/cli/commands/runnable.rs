#[cfg(feature = "online")]
use tokio::runtime::Builder;

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
        let rt = Builder::new_current_thread().enable_all().build()?;

        rt.block_on(self.run_async())
    }
}
