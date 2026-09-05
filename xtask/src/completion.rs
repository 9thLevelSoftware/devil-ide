pub mod artifact_files;
pub mod links;
pub mod schema;
pub mod structure;

/// Returns whether a tuple is eligible for product evidence consideration.
///
/// This is an eligibility primitive only. It does not establish evidence
/// authenticity and does not produce a release verdict.
pub fn qualifies_as_product_evidence(
    layer: &str,
    input_route: &str,
    result: &str,
    required_dependency_substituted: bool,
) -> bool {
    layer == "product"
        && input_route == "native-input"
        && result == "passed"
        && !required_dependency_substituted
}
