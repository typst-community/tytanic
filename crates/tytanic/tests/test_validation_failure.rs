mod fixture;

#[test]
fn test_manifest_invalid() {
    let env = fixture::Environment::default_package();
    std::fs::remove_dir_all(env.root().join("template")).unwrap();
    let res = env.run_tytanic(["status"]);

    insta::assert_snapshot!(res.output(), @r#"
    --- CODE: 2
    --- STDOUT:

    --- STDERR:
    error: Failed to validate manifest:
           `template.entrypoint`: the path did not exist: "main.typ" ("<TEMP_DIR>/template/main.typ")
           `template.path`: the path did not exist: "template" ("<TEMP_DIR>/template")

    --- END
    "#);
}
