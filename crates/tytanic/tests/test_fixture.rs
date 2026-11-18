mod fixture;

/// Checks if the tests listed in the fixture are up to date with the default
/// package fixture.
#[test]
fn test_default_package_tests_up_to_date() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["list"]);

    assert!(res.output().status().success(), "{}", res.output());

    let ids: Vec<_> = res
        .output()
        .stderr()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert_eq!(ids, fixture::DEFAULT_PACKAGE_TESTS, "{}", res.output());
}
