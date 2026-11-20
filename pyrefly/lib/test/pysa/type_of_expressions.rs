/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::BTreeMap;

use dupe::Dupe;
use pretty_assertions::assert_eq;

use crate::report::pysa::context::ModuleAnswersContext;
use crate::report::pysa::context::ModuleContext;
use crate::report::pysa::context::PysaResolver;
use crate::report::pysa::module::ModuleIds;
use crate::report::pysa::type_of_expression::ExportedExpressions;
use crate::report::pysa::type_of_expression::export_type_of_expressions;
use crate::report::pysa::types::PysaType;
use crate::test::pysa::utils::create_state;
use crate::test::pysa::utils::get_handle_for_module_name;

/// The exported type of each location, over every function of the module.
fn type_of_expressions(
    python_code: &str,
    expressions: ExportedExpressions,
) -> BTreeMap<String, PysaType> {
    let state = create_state("test", python_code);
    let transaction = state.transaction();
    let handles = transaction.handles();
    let module_ids = ModuleIds::new(&handles);
    let handle = get_handle_for_module_name("test", &transaction);
    let resolver = PysaResolver::new_for_test(&transaction, &module_ids, handle.dupe(), &handles);
    let context = ModuleContext {
        answers_context: ModuleAnswersContext::create(handle.dupe(), &transaction, &module_ids),
        resolver: &resolver,
    };
    export_type_of_expressions(&context, expressions)
        .into_values()
        .flat_map(|function| {
            function
                .locations
                .into_iter()
                .map(|(location, id)| {
                    (
                        location.as_key(),
                        function.type_table[id.0 as usize].clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The exported type string of each location.
fn type_strings(python_code: &str, expressions: ExportedExpressions) -> BTreeMap<String, String> {
    type_of_expressions(python_code, expressions)
        .into_iter()
        .map(|(location, pysa_type)| (location, pysa_type.string))
        .collect()
}

fn expected(locations: &[(&str, &str)]) -> BTreeMap<String, String> {
    locations
        .iter()
        .map(|(location, string)| ((*location).to_owned(), (*string).to_owned()))
        .collect()
}

const CODE: &str = r#"
def f(x: int, xs: list[int]) -> int:
    return (x + 1) * xs[0]
"#;

#[test]
fn test_export_pysa_expressions() {
    assert_eq!(
        expected(&[("3:13-3:14", "int"), ("3:22-3:24", "list[int]")]),
        type_strings(CODE, ExportedExpressions::PysaNeeds),
    );
}

#[test]
fn test_export_all_expressions() {
    assert_eq!(
        expected(&[
            ("3:12-3:27", "int"),
            ("3:13-3:14", "int"),
            ("3:13-3:18", "int"),
            ("3:17-3:18", "int"),
            ("3:22-3:24", "list[int]"),
            ("3:22-3:27", "int"),
            ("3:25-3:26", "int"),
        ]),
        type_strings(CODE, ExportedExpressions::All),
    );
}

#[test]
fn test_export_all_expressions_in_annotations() {
    assert_eq!(
        // Annotations are types, not values, so they have none; nor has a name being bound.
        expected(&[
            ("3:14-3:16", "str"),
            ("4:12-4:15", "list[int]"),
            ("4:13-4:14", "int"),
        ]),
        type_strings(
            r#"
def f(x: int) -> list[int]:
    y: str = ""
    return [x]
"#,
            ExportedExpressions::All
        ),
    );
}

#[test]
fn test_export_all_expressions_in_decorators() {
    assert_eq!(
        expected(&[
            ("3:12-3:13", "object"),
            ("4:2-4:3", "(g: object) -> object")
        ]),
        type_strings(
            r#"
def d(g: object) -> object:
    return g
@d
def f() -> None:
    pass
"#,
            ExportedExpressions::All
        ),
    );
}

#[test]
fn test_export_all_expressions_in_lambdas_and_comprehensions() {
    // `x + 1` in the lambda's body has no type trace, so it has no type.
    assert_eq!(
        expected(&[
            ("3:19-3:20", "Unknown"),
            ("3:23-3:24", "int"),
            ("3:9-3:24", "(x: Unknown) -> Unknown"),
            ("4:20-4:22", "list[int]"),
            ("4:5-4:23", "list[Unknown]"),
            ("4:6-4:10", "Unknown"),
            ("4:6-4:7", "(x: Unknown) -> Unknown"),
            ("4:8-4:9", "int"),
        ]),
        type_strings(
            r#"
def f(xs: list[int]) -> None:
    g = lambda x: x + 1
    [g(x) for x in xs]
"#,
            ExportedExpressions::All
        ),
    );
}
