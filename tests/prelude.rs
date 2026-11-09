//! The prelude must be enough to write a signature and build an error without
//! naming `casket::error` directly.

// Deliberately infallible: the point is that the prelude supplies `Result` in a
// signature, which is what callers write. Clippy's `unnecessary_wraps` is the
// wrong lint to obey here.
#[allow(clippy::unnecessary_wraps)]
fn takes() -> casket::prelude::Result<()> {
    Ok(())
}

#[test]
fn prelude_result_is_the_crate_result() {
    let ok: casket::prelude::Result<u8> = takes().map(|()| 7u8);
    assert_eq!(ok.expect("infallible"), 7);
}

#[test]
fn prelude_exports_the_error_type() {
    let e = casket::prelude::Error::Unsupported("x".into());
    assert!(e.to_string().contains("unsupported"));
}

#[test]
fn prelude_error_is_the_crate_error() {
    let e: casket::Error = casket::prelude::Error::Unsupported("y".into());
    assert!(matches!(e, casket::Error::Unsupported(_)));
}
