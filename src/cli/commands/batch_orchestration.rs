/// The scientific outcome for one item in an ordered batch.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BatchExecutionOutcome<T, E> {
    Success(T),
    NonConverged(T),
    Error(E),
}

/// Execute each item once, in iterator order. Infrastructure errors stop the batch;
/// item errors are retained as outcomes so later calculations still run.
pub(crate) fn execute_batch<I, T, E, Execute, IsConverged, IsInfrastructure>(
    items: I,
    mut execute: Execute,
    mut is_converged: IsConverged,
    mut is_infrastructure: IsInfrastructure,
) -> Result<Vec<BatchExecutionOutcome<T, E>>, E>
where
    I: IntoIterator,
    Execute: FnMut(I::Item) -> Result<T, E>,
    IsConverged: FnMut(&T) -> bool,
    IsInfrastructure: FnMut(&E) -> bool,
{
    let mut outcomes = Vec::new();
    for item in items {
        match execute(item) {
            Ok(result) if is_converged(&result) => {
                outcomes.push(BatchExecutionOutcome::Success(result));
            }
            Ok(result) => outcomes.push(BatchExecutionOutcome::NonConverged(result)),
            Err(error) if is_infrastructure(&error) => return Err(error),
            Err(error) => outcomes.push(BatchExecutionOutcome::Error(error)),
        }
    }
    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::{execute_batch, BatchExecutionOutcome};

    #[test]
    fn consumes_in_order_continues_item_errors_and_stops_infrastructure_errors() {
        let mut seen = Vec::new();
        let outcomes = execute_batch(
            [1, 2, 3, 4],
            |item| {
                seen.push(item);
                match item {
                    2 => Err("calculation"),
                    4 => Err("output"),
                    value => Ok((value, value != 3)),
                }
            },
            |(_, converged)| *converged,
            |error| *error == "output",
        );

        assert_eq!(seen, [1, 2, 3, 4]);
        assert_eq!(outcomes, Err("output"));

        let outcomes = execute_batch(
            [1, 2, 3],
            |item| match item {
                2 => Err("calculation"),
                value => Ok((value, value != 3)),
            },
            |(_, converged)| *converged,
            |_| false,
        )
        .unwrap();
        assert_eq!(
            outcomes,
            [
                BatchExecutionOutcome::Success((1, true)),
                BatchExecutionOutcome::Error("calculation"),
                BatchExecutionOutcome::NonConverged((3, false)),
            ]
        );
    }
}
