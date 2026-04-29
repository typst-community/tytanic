mod fixture;

#[test]
fn test_update_ephemeral_to_persistent_238() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["update", "--no-optimize-refs", "failing/persistent-empty"]);

    // TODO: This should assert on the output in some way but we don't have
    // reproducible test runs yet.
    assert!(res.output().status().success());
}
