use integration_utils::integration_contract::IntegrationContract;
use near_workspaces::result::{ExecutionFailure, ExecutionFinalResult, ExecutionResult, ExecutionSuccess};
use sweat_model::SweatApiIntegration;

use crate::context::{prepare_contract, IntegrationContext};

pub(crate) trait PanicFinder {
    fn has_panic(&self, message: &str) -> bool;
}

impl PanicFinder for Result<ExecutionSuccess, ExecutionFailure> {
    fn has_panic(&self, message: &str) -> bool {
        match self {
            Ok(ok) => ok.has_panic(message),
            Err(err) => err.has_panic(message),
        }
    }
}

impl<T> PanicFinder for ExecutionResult<T> {
    fn has_panic(&self, message: &str) -> bool {
        self.outcomes()
            .into_iter()
            .map(|item| match item.clone().into_result() {
                Ok(_) => None,
                Err(err) => Some(err),
            })
            .any(|item| match item {
                None => false,
                Some(err) => format!("{err:?}").contains(message),
            })
    }
}

pub(crate) fn log_result(result: ExecutionFinalResult) {
    let result = result.into_result();
    println!("  📬 Result: {result:?}");

    if let Ok(result) = result {
        for log in result.logs() {
            println!("  📖 {log}");
        }
    }
}
