//! The check registry. Add a module per check and list it in [`all`].

use crate::Check;

pub mod given_when_then;

/// Every registered check, in report order.
pub fn all() -> Vec<Box<dyn Check>> {
    vec![Box::new(given_when_then::GivenWhenThen)]
}
