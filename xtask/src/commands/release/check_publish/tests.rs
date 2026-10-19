use serde_json::{Value, json};

use super::validate;

fn package(name: &str, publish: &Value, dependencies: &Value) -> Value {
    json!({
        "name": name,
        "publish": publish,
        "dependencies": dependencies,
    })
}

fn dependency(name: &str, req: &str, kind: &Value) -> Value {
    json!({
        "name": name,
        "req": req,
        "path": format!("/workspace/{name}"),
        "kind": kind,
    })
}

#[test]
fn rejects_unpublished_path_dependency_of_published_package() {
    let metadata = json!({
        "packages": [
            package(
                "openhid-cli",
                &Value::Null,
                &json!([dependency("openhid-ipc", "^0.7.7", &Value::Null)]),
            ),
            package("openhid-ipc", &json!([]), &json!([])),
        ],
    });

    let error = validate(&metadata.to_string()).unwrap_err().to_string();

    assert!(error.contains("`openhid-cli` depends on unpublished path package `openhid-ipc`"));
}

#[test]
fn rejects_path_dependency_without_registry_version() {
    let metadata = json!({
        "packages": [
            package(
                "openhid-ipc",
                &Value::Null,
                &json!([dependency("openhid-core", "*", &Value::Null)]),
            ),
            package("openhid-core", &Value::Null, &json!([])),
        ],
    });

    let error = validate(&metadata.to_string()).unwrap_err().to_string();

    assert!(error.contains(
        "`openhid-ipc` path dependency on `openhid-core` must declare a registry version"
    ));
}

#[test]
fn accepts_publishable_normal_and_build_closure_and_ignores_dev_dependencies() {
    let metadata = json!({
        "packages": [
            package(
                "openhid-cli",
                &Value::Null,
                &json!([
                    dependency("openhid-core", "^0.7.8", &Value::Null),
                    dependency("openhid-build", "^0.7.8", &json!("build")),
                    dependency("openhid-test", "*", &json!("dev")),
                ]),
            ),
            package("openhid-core", &Value::Null, &json!([])),
            package("openhid-build", &Value::Null, &json!([])),
            package("openhid-test", &json!([]), &json!([])),
        ],
    });

    assert_eq!(validate(&metadata.to_string()).unwrap(), 3);
}
