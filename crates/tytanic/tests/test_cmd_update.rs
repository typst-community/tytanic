mod fixture;

#[test]
fn test_update_ephemeral_to_persistent_238() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["update", "--no-optimize-refs", "failing/persistent-empty"]);

    // Run metadata isn't reproducible, so only assert stable output fragments.
    assert!(res.output().status().success(), "{}", res.output());
    assert!(
        !res.output().stderr().contains("Optimizing references"),
        "{}",
        res.output()
    );
}

#[test]
fn test_update_reports_reference_optimization_220() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["update", "failing/persistent-empty"]);

    assert!(res.output().status().success(), "{}", res.output());
    assert!(
        res.output()
            .stderr()
            .contains("hint: Optimizing references may take much longer than tt run"),
        "{}",
        res.output()
    );
    assert!(
        res.output().stderr().contains(
            "Consider using --no-optimize-refs when using third-party hosting for references"
        ),
        "{}",
        res.output()
    );
}
