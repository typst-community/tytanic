mod fixture;

/// The tests of the default package fixture in suite order.
const TESTS: [&str; 11] = [
    "@template",
    "failing/compile",
    "failing/ephemeral-compare-failure",
    "failing/ephemeral-compile-failure",
    "failing/persistent-compare-failure",
    "failing/persistent-compile-failure",
    "failing/persistent-empty",
    "passing/compile",
    "passing/ephemeral",
    "passing/input-merge",
    "passing/persistent",
];

/// Returns the annotations and ids of the reported tests in the order they
/// were reported in.
fn reported(stderr: &str) -> Vec<(&str, &str)> {
    stderr
        .lines()
        .filter_map(|line| {
            let (annot, rest) = line.trim_start().split_once(" [")?;
            let (_, id) = rest.split_once("] ")?;
            matches!(annot, "fail" | "pass" | "compile").then_some((annot, id))
        })
        .collect()
}

#[test]
fn test_run_single_job_reports_in_suite_order() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["--jobs", "1", "run", "--no-fail-fast"]);

    // Run metadata isn't reproducible, so only assert stable output fragments.
    assert_eq!(res.output().status().code(), Some(1), "{}", res.output());

    let ids: Vec<_> = reported(res.output().stderr())
        .into_iter()
        .map(|(_, id)| id)
        .collect();
    assert_eq!(ids, TESTS, "{}", res.output());
}

#[test]
fn test_run_multiple_jobs_reports_all_tests() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["--jobs", "4", "run", "--no-fail-fast"]);

    assert_eq!(res.output().status().code(), Some(1), "{}", res.output());
    assert!(
        res.output()
            .stderr()
            .contains("11/11 tests run: 4 passed, 7 failed"),
        "{}",
        res.output()
    );

    // Tests are reported in the order they finish in.
    let mut ids: Vec<_> = reported(res.output().stderr())
        .into_iter()
        .map(|(_, id)| id)
        .collect();
    ids.sort();
    assert_eq!(ids, TESTS, "{}", res.output());
}

#[test]
fn test_run_single_job_fail_fast() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["--jobs", "1", "run", "--fail-fast"]);

    assert_eq!(res.output().status().code(), Some(1), "{}", res.output());
    assert!(
        res.output()
            .stderr()
            .contains("1/11 tests run: 0 passed, 1 failed, 10 skipped"),
        "{}",
        res.output()
    );
    assert_eq!(
        reported(res.output().stderr()),
        [("fail", "@template")],
        "{}",
        res.output()
    );
}

#[test]
fn test_run_multiple_jobs_fail_fast() {
    let env = fixture::Environment::default_package();
    let res = env.run_tytanic(["--jobs", "4", "run", "--fail-fast"]);

    assert_eq!(res.output().status().code(), Some(1), "{}", res.output());

    // Tests which were already running are still reported, so there may be more
    // than one, but each of them only once.
    let reported = reported(res.output().stderr());
    assert!(
        reported.iter().any(|(annot, _)| *annot == "fail"),
        "{}",
        res.output()
    );

    let mut ids: Vec<_> = reported.iter().map(|(_, id)| *id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), reported.len(), "{}", res.output());
    assert!(ids.iter().all(|id| TESTS.contains(id)), "{}", res.output());
}
