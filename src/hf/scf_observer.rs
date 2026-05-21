use super::scf_iteration::ScfIteration;

pub trait ScfObserver {
    fn on_iteration(&mut self, iteration: &ScfIteration);
}

impl ScfObserver for Box<dyn ScfObserver> {
    fn on_iteration(&mut self, iteration: &ScfIteration) {
        self.as_mut().on_iteration(iteration);
    }
}

#[allow(dead_code)]
pub struct NoopScfObserver;

impl ScfObserver for NoopScfObserver {
    fn on_iteration(&mut self, _iteration: &ScfIteration) {}
}
