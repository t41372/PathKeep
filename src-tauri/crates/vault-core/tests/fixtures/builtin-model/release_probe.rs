// Compile the production URL selector with release cfg, independent of dependency profiles.
#[path = "../../../src/ai/model_source.rs"]
mod model_source;
fn main() {
    // This exhaustive match also fails to compile if the fixture variant leaks into release.
    let model_source::ModelSource::Hub = model_source::ModelSource::configured().unwrap();
    assert_eq!(
        model_source::ModelSource::Hub.url("org/model", "weights"),
        "https://huggingface.co/org/model/resolve/main/weights"
    );
}
