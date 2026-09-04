//! TSQ1 runtime boundary for shared typed calculation semantics.
//!
//! TSQ1 continues to own timeline data, references, and serialization. This
//! module exposes the shared scalar/function layer without adding parser or
//! workbook responsibilities to the sequence format.

pub use openformula_kernel::{
    Argument, CalcError, CalcErrorKind, CalcResult, CoercionPolicy, EvalContext, FunctionMetadata,
    Number, PureContext, Value,
};

use openformula_kernel::FunctionRegistry;

/// A TSQ1-owned handle to the version-pinned shared calculation registry.
#[derive(Clone)]
pub struct CalculationKernel {
    registry: FunctionRegistry,
}

impl CalculationKernel {
    /// Construct the OpenFormula-facing standard registry.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            registry: FunctionRegistry::standard(),
        }
    }

    /// Construct a registry with an explicit coercion policy.
    #[must_use]
    pub fn with_coercion_policy(policy: CoercionPolicy) -> Self {
        Self {
            registry: FunctionRegistry::standard_with_policy(policy),
        }
    }

    /// Evaluate a function after TSQ1's runtime has resolved its arguments.
    pub fn evaluate(
        &self,
        name: &str,
        arguments: &[Argument],
        context: &mut dyn EvalContext,
    ) -> CalcResult {
        self.registry.evaluate(name, arguments, context)
    }

    /// Access the registry so a host can add explicitly namespaced extensions.
    #[must_use]
    pub fn registry_mut(&mut self) -> &mut FunctionRegistry {
        &mut self.registry
    }

    /// Return compatibility metadata for a standard or extension function.
    #[must_use]
    pub fn metadata(&self, name: &str) -> Option<&FunctionMetadata> {
        self.registry.metadata(name)
    }
}

impl Default for CalculationKernel {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_the_shared_contract_without_timeline_coupling() {
        let kernel = CalculationKernel::standard();
        let result = kernel
            .evaluate(
                "ROUND",
                &[
                    Argument::scalar(Value::Number(
                        Number::try_from_f64(1.005).expect("finite fixture"),
                    )),
                    Argument::scalar(2_i64),
                ],
                &mut PureContext,
            )
            .expect("shared calculation");
        let Value::Number(result) = result else {
            panic!("ROUND must return a number");
        };
        assert_eq!(result.as_f64(), 1.01);
        assert_eq!(
            kernel
                .metadata("ROUND")
                .and_then(FunctionMetadata::openformula_reference),
            Some("6.17.5")
        );
    }
}
