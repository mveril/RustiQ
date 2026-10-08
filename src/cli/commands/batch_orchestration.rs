/// An item that owns the context needed for one execution.
pub(crate) trait Executable {
    type Output;
    type Error: ExecutionError;

    fn execute(self) -> Result<Self::Output, Self::Error>;
}

pub(crate) trait ExecutionError {
    fn is_fatal(&self) -> bool;
}

/// Classification used by consumers, independently of batch execution.
pub(crate) trait ExecutionResult {
    fn is_converged(&self) -> bool;
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct BatchSummary {
    pub succeeded: usize,
    pub non_converged: usize,
    pub failed: usize,
}

impl BatchSummary {
    pub(crate) fn from_results<T: ExecutionResult, E>(results: &[Result<T, E>]) -> Self {
        let mut summary = Self::default();
        for result in results {
            match result {
                Ok(result) if result.is_converged() => summary.succeeded += 1,
                Ok(_) => summary.non_converged += 1,
                Err(_) => summary.failed += 1,
            }
        }
        summary
    }
}

pub(crate) type BatchExecutionResult<T, E> = Result<Vec<Result<T, E>>, E>;

/// Execute each item once, in iterator order. Fatal errors stop the batch;
/// recoverable errors are retained so later items still run.
pub(crate) fn execute_batch<I, Item>(items: I) -> BatchExecutionResult<Item::Output, Item::Error>
where
    I: IntoIterator<Item = Item>,
    Item: Executable,
{
    let mut results = Vec::new();
    for item in items {
        let result = item.execute();
        match result {
            Err(error) if error.is_fatal() => return Err(error),
            result => results.push(result),
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{execute_batch, BatchSummary, Executable, ExecutionError, ExecutionResult};

    #[derive(Debug, PartialEq, Eq)]
    enum TestError {
        Recoverable,
        Fatal,
    }

    impl ExecutionError for TestError {
        fn is_fatal(&self) -> bool {
            matches!(self, Self::Fatal)
        }
    }

    struct Task<'a> {
        id: usize,
        result: Result<usize, TestError>,
        seen: &'a RefCell<Vec<usize>>,
    }

    impl Executable for Task<'_> {
        // Deliberately does not implement ExecutionResult.
        type Output = usize;
        type Error = TestError;

        fn execute(self) -> Result<Self::Output, Self::Error> {
            self.seen.borrow_mut().push(self.id);
            self.result
        }
    }

    #[test]
    fn preserves_order_and_continues_after_recoverable_errors() {
        let seen = RefCell::new(Vec::new());
        let tasks = [Ok(1), Err(TestError::Recoverable), Ok(3)]
            .into_iter()
            .enumerate()
            .map(|(id, result)| Task {
                id,
                result,
                seen: &seen,
            });
        assert_eq!(
            execute_batch(tasks).unwrap(),
            [Ok(1), Err(TestError::Recoverable), Ok(3)]
        );
        assert_eq!(*seen.borrow(), [0, 1, 2]);
    }

    #[test]
    fn fatal_error_leaves_later_items_unexecuted() {
        let seen = RefCell::new(Vec::new());
        let tasks = [
            Ok(1),
            Err(TestError::Recoverable),
            Err(TestError::Fatal),
            Ok(4),
        ]
        .into_iter()
        .enumerate()
        .map(|(id, result)| Task {
            id,
            result,
            seen: &seen,
        });
        assert_eq!(execute_batch(tasks), Err(TestError::Fatal));
        assert_eq!(*seen.borrow(), [0, 1, 2]);
    }

    #[test]
    fn empty_batch_returns_no_results() {
        assert_eq!(execute_batch(std::iter::empty::<Task<'_>>()).unwrap(), []);
    }

    struct TestResult(bool);

    impl ExecutionResult for TestResult {
        fn is_converged(&self) -> bool {
            self.0
        }
    }

    #[test]
    fn summary_counts_results_without_affecting_execution() {
        let results = [
            Ok(TestResult(true)),
            Ok(TestResult(false)),
            Err(TestError::Recoverable),
            Ok(TestResult(true)),
        ];
        assert_eq!(
            BatchSummary::from_results(&results),
            BatchSummary {
                succeeded: 2,
                non_converged: 1,
                failed: 1
            }
        );
        assert_eq!(
            BatchSummary::from_results::<TestResult, TestError>(&[]),
            BatchSummary::default()
        );
    }
}
