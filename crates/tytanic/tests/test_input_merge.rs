mod fixture;

/// The `input` annotation is merged with the project-wide defaults from the
/// `default.inputs` config key, where annotations with the same key override
/// the project-wide config.
#[test]
fn test_input_annotation_overrides_default_inputs() {
    let env = fixture::Environment::default_package();

    let res = env.run_tytanic(["run", "passing/input-merge"]);

    assert!(res.output().status().success(), "{}", res.output());
}
