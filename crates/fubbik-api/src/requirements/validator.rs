//! Direct port of `packages/api/src/requirements/validator.ts`'s
//! `validateSteps` — enforces BDD phase ordering (given -> when -> then,
//! with `and`/`but` inheriting whatever phase precedes them) on a
//! requirement's step list. Pure function, no SQL, no I/O.

use fubbik_db::repo::requirement::{RequirementStep, StepKeyword};

/// One validation failure. `step` is the zero-based index into the input
/// steps array, or `-1` for the two whole-sequence checks ("must contain a
/// `when`"/"must contain a `then`") — matches Node's `StepError` exactly
/// (`packages/api/src/requirements/validator.ts:3-6`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct StepError {
    pub step: i32,
    pub error: String,
}

#[derive(PartialEq, Eq)]
enum Phase {
    Given,
    When,
    Then,
}

/// Matches Node's `validateSteps` algorithm and error text verbatim
/// (`packages/api/src/requirements/validator.ts:10-66`).
pub fn validate_steps(steps: &[RequirementStep]) -> Vec<StepError> {
    let mut errors = Vec::new();

    if steps.is_empty() {
        errors.push(StepError {
            step: 0,
            error: "Must have at least one step".to_string(),
        });
        return errors;
    }

    let first_keyword = steps[0].keyword;
    if first_keyword == StepKeyword::And || first_keyword == StepKeyword::But {
        errors.push(StepError {
            step: 0,
            error: "First step cannot be 'and' or 'but'".to_string(),
        });
    } else if first_keyword != StepKeyword::Given {
        errors.push(StepError {
            step: 0,
            error: "First step must be 'given'".to_string(),
        });
    }

    let mut phase = Phase::Given;

    for (i, step) in steps.iter().enumerate() {
        match step.keyword {
            StepKeyword::And | StepKeyword::But => {
                // Inherits the current phase, no transition — already
                // reported above if this is index 0.
                continue;
            }
            StepKeyword::Given => {
                if phase == Phase::When || phase == Phase::Then {
                    errors.push(StepError {
                        step: i as i32,
                        error: "Cannot use 'given' after 'when' or 'then'".to_string(),
                    });
                }
            }
            StepKeyword::When => {
                if phase == Phase::Then {
                    errors.push(StepError {
                        step: i as i32,
                        error: "Cannot use 'when' after 'then'".to_string(),
                    });
                } else {
                    phase = Phase::When;
                }
            }
            StepKeyword::Then => {
                if phase == Phase::Given {
                    errors.push(StepError {
                        step: i as i32,
                        error: "'then' must come after 'when' phase".to_string(),
                    });
                } else {
                    phase = Phase::Then;
                }
            }
        }
    }

    let has_when = steps.iter().any(|s| s.keyword == StepKeyword::When);
    let has_then = steps.iter().any(|s| s.keyword == StepKeyword::Then);

    if !has_when {
        errors.push(StepError {
            step: -1,
            error: "Must contain at least one 'when' step".to_string(),
        });
    }
    if !has_then {
        errors.push(StepError {
            step: -1,
            error: "Must contain at least one 'then' step".to_string(),
        });
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(keyword: StepKeyword) -> RequirementStep {
        RequirementStep {
            keyword,
            text: "x".to_string(),
            params: None,
        }
    }

    #[test]
    fn empty_steps_is_the_only_error() {
        // Given the inline inputs and test fixtures.
        // When
        let errors = validate_steps(&[]);
        // Then
        assert_eq!(
            errors,
            vec![StepError {
                step: 0,
                error: "Must have at least one step".to_string()
            }]
        );
    }

    #[test]
    fn valid_given_when_then_has_no_errors() {
        // Given the inline inputs and test fixtures.
        // When
        let steps = [
            step(StepKeyword::Given),
            step(StepKeyword::When),
            step(StepKeyword::Then),
        ];
        // Then
        assert!(validate_steps(&steps).is_empty());
    }

    #[test]
    fn first_step_must_be_given() {
        // Given
        let steps = [step(StepKeyword::When), step(StepKeyword::Then)];
        // When
        let errors = validate_steps(&steps);
        // Then
        assert!(
            errors
                .iter()
                .any(|e| e.step == 0 && e.error == "First step must be 'given'")
        );
    }

    #[test]
    fn first_step_cannot_be_and_or_but() {
        // Given
        let steps = [
            step(StepKeyword::And),
            step(StepKeyword::When),
            step(StepKeyword::Then),
        ];
        // When
        let errors = validate_steps(&steps);
        // Then
        assert!(
            errors
                .iter()
                .any(|e| e.step == 0 && e.error == "First step cannot be 'and' or 'but'")
        );
    }

    #[test]
    fn given_after_when_is_rejected() {
        // Given
        let steps = [
            step(StepKeyword::Given),
            step(StepKeyword::When),
            step(StepKeyword::Given),
            step(StepKeyword::Then),
        ];
        // When
        let errors = validate_steps(&steps);
        // Then
        assert!(
            errors
                .iter()
                .any(|e| e.step == 2 && e.error.contains("Cannot use 'given'"))
        );
    }

    #[test]
    fn when_after_then_is_rejected() {
        // Given
        let steps = [
            step(StepKeyword::Given),
            step(StepKeyword::When),
            step(StepKeyword::Then),
            step(StepKeyword::When),
        ];
        // When
        let errors = validate_steps(&steps);
        // Then
        assert!(
            errors
                .iter()
                .any(|e| e.step == 3 && e.error.contains("Cannot use 'when'"))
        );
    }

    #[test]
    fn then_before_when_is_rejected() {
        // Given
        let steps = [step(StepKeyword::Given), step(StepKeyword::Then)];
        // When
        let errors = validate_steps(&steps);
        // Then
        assert!(
            errors
                .iter()
                .any(|e| e.step == 1 && e.error.contains("must come after 'when'"))
        );
    }

    #[test]
    fn missing_when_and_then_are_both_reported() {
        // Given
        let steps = [step(StepKeyword::Given)];
        // When
        let errors = validate_steps(&steps);
        // Then
        assert!(
            errors
                .iter()
                .any(|e| e.step == -1 && e.error.contains("'when'"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.step == -1 && e.error.contains("'then'"))
        );
    }

    #[test]
    fn and_but_inherit_phase_without_transition() {
        // Given the inline inputs and test fixtures.
        // When
        let steps = [
            step(StepKeyword::Given),
            step(StepKeyword::And),
            step(StepKeyword::When),
            step(StepKeyword::But),
            step(StepKeyword::Then),
        ];
        // Then
        assert!(validate_steps(&steps).is_empty());
    }
}
