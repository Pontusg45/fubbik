//! The check registry. Add a module per check and list it in [`all`].

use crate::Check;

/// Every registered check, in report order.
pub fn all() -> Vec<Box<dyn Check>> {
    vec![]
}
