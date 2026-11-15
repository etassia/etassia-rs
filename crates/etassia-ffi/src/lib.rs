//! C ABI: etassia_render / etassia_detect / etassia_free.
//!
//! Every call returns a heap buffer the caller releases with `etassia_free`.
//! `status` (out) is 0 on success (buffer = SVG / type name), 1 on error
//! (buffer = JSON `{class,message,line,col}`; class "Unported" means use the
//! Ruby fallback). `out_len` receives the byte length (buffer is also NUL-terminated).
use etassia::Error;
use std::os::raw::{c_char, c_int};

#[repr(C)]
pub struct EtassiaBuf {
    pub ptr: *mut u8,
    pub len: usize,
    pub status: c_int,
}

fn into_buf(r: Result<String, Error>) -> EtassiaBuf {
    let (mut bytes, status) = match r {
        Ok(s) => (s.into_bytes(), 0),
        Err(e) => (e.to_json().into_bytes(), 1),
    };
    let len = bytes.len();
    bytes.push(0);
    let boxed = bytes.into_boxed_slice();
    EtassiaBuf {
        ptr: Box::into_raw(boxed) as *mut u8,
        len,
        status,
    }
}

unsafe fn src<'a>(ptr: *const c_char, len: usize) -> Result<&'a str, Error> {
    if ptr.is_null() {
        return Err(Error::Parse {
            class: "InvalidArgument".into(),
            message: "null source".into(),
            line: 0,
            col: 0,
        });
    }
    std::str::from_utf8(std::slice::from_raw_parts(ptr as *const u8, len)).map_err(|e| {
        Error::Parse {
            class: "InvalidEncoding".into(),
            message: e.to_string(),
            line: 0,
            col: 0,
        }
    })
}

/// Render `src` (UTF-8, `len` bytes). `opts_json` is reserved (may be NULL).
///
/// # Safety
/// `src_ptr` must point to `len` readable bytes; `opts_json` NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn etassia_render(
    src_ptr: *const c_char,
    len: usize,
    _opts_json: *const c_char,
) -> EtassiaBuf {
    into_buf(src(src_ptr, len).and_then(etassia::render))
}

/// Detect the diagram type name of `src`.
///
/// # Safety
/// `src_ptr` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn etassia_detect(src_ptr: *const c_char, len: usize) -> EtassiaBuf {
    into_buf(src(src_ptr, len).and_then(|s| {
        etassia::detect(s).ok_or_else(|| Error::Parse {
            class: "DiagramTypeError".into(),
            message: "cannot detect diagram type".into(),
            line: 0,
            col: 0,
        })
    }))
}

fn dim_src<'a>(ptr: *const c_char, len: usize) -> Result<&'a str, etassia_dim::DimError> {
    if ptr.is_null() {
        return Err(etassia_dim::DimError::Json("null source".into()));
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) })
        .map_err(|e| etassia_dim::DimError::Json(e.to_string()))
}

/// Render a DIM 0.1 document (UTF-8, `len` bytes) to SVG. Same buffer
/// contract as `etassia_render`; errors carry the classes `DimError`,
/// `Unsupported`, and `LayoutError`. The export is additive: existing
/// consumers of the three original functions are unaffected.
///
/// # Safety
/// `src_ptr` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn etassia_render_dim(src_ptr: *const c_char, len: usize) -> EtassiaBuf {
    into_buf_dim(dim_src(src_ptr, len).and_then(etassia_dim::render))
}

/// Release a buffer returned by this library.
///
/// # Safety
/// `buf` must come from `etassia_render`/`etassia_detect`/`etassia_render_dim`, freed at most once.
#[no_mangle]
pub unsafe extern "C" fn etassia_free(buf: EtassiaBuf) {
    if !buf.ptr.is_null() {
        drop(Box::from_raw(std::slice::from_raw_parts_mut(
            buf.ptr,
            buf.len + 1,
        )));
    }
}

fn into_buf_dim(r: Result<String, etassia_dim::DimError>) -> EtassiaBuf {
    let (mut bytes, status) = match r {
        Ok(s) => (s.into_bytes(), 0),
        Err(e) => (e.to_json().into_bytes(), 1),
    };
    let len = bytes.len();
    bytes.push(0);
    let boxed = bytes.into_boxed_slice();
    EtassiaBuf {
        ptr: Box::into_raw(boxed) as *mut u8,
        len,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_unported_then_free() {
        let s = "pie\n  \"a\": 1\n";
        let buf = unsafe { etassia_render(s.as_ptr() as *const c_char, s.len(), std::ptr::null()) };
        let body = unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(buf.ptr, buf.len))
                .unwrap()
                .to_string()
        };
        // Either SVG (status 0) or a JSON error (status 1); both must be well-formed.
        if buf.status == 0 {
            assert!(body.starts_with("<svg"));
        } else {
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert!(v["class"].is_string());
        }
        unsafe { etassia_free(buf) };
    }

    #[test]
    fn null_source_is_error() {
        let buf = unsafe { etassia_render(std::ptr::null(), 0, std::ptr::null()) };
        assert_eq!(buf.status, 1);
        unsafe { etassia_free(buf) };
    }

    #[test]
    fn dim_document_renders_over_the_abi() {
        let doc = r#"{
          "dim": "0.1", "notation": "generic",
          "graph": {
            "id": "root",
            "layoutOptions": {"elk.algorithm": "layered", "elk.direction": "RIGHT"},
            "children": [
              {"id": "n1", "width": 120, "height": 60,
               "content": [{"kind": "label", "text": "Person", "role": "title"}]},
              {"id": "n2", "width": 120, "height": 60,
               "content": [{"kind": "label", "text": "Address", "role": "title"}]}
            ],
            "edges": [
              {"id": "e1", "sources": ["n1"], "targets": ["n2"],
               "style": "association", "label": {"text": "lives at"}}
            ]
          }
        }"#;
        let buf = unsafe { etassia_render_dim(doc.as_ptr() as *const c_char, doc.len()) };
        assert_eq!(buf.status, 0, "DIM render failed");
        let body =
            unsafe { std::str::from_utf8(std::slice::from_raw_parts(buf.ptr, buf.len)).unwrap() };
        assert!(body.starts_with("<svg"));
        assert!(body.contains(">Person<"));
        assert!(body.contains(">lives at<"));
        // the laid-out second node carries its coordinates
        assert!(body.contains(r#"<g id="n2">"#));
        unsafe { etassia_free(buf) };
    }

    #[test]
    fn dim_wrong_version_is_unsupported() {
        let doc = r#"{"dim": "9.9", "notation": "generic", "graph": {"id": "root"}}"#;
        let buf = unsafe { etassia_render_dim(doc.as_ptr() as *const c_char, doc.len()) };
        assert_eq!(buf.status, 1);
        let body =
            unsafe { std::str::from_utf8(std::slice::from_raw_parts(buf.ptr, buf.len)).unwrap() };
        assert!(body.contains("\"Unsupported\""));
        unsafe { etassia_free(buf) };
    }

    #[test]
    fn dim_determinism_over_the_abi() {
        let doc = r#"{"dim":"0.1","notation":"generic","graph":{"id":"root","children":[{"id":"a","width":60,"height":40},{"id":"b","width":60,"height":40}],"edges":[{"id":"e","sources":["a"],"targets":["b"]}]}}"#;
        let b1 = unsafe { etassia_render_dim(doc.as_ptr() as *const c_char, doc.len()) };
        let b2 = unsafe { etassia_render_dim(doc.as_ptr() as *const c_char, doc.len()) };
        let s1 = unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(b1.ptr, b1.len))
                .unwrap()
                .to_string()
        };
        let s2 = unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(b2.ptr, b2.len))
                .unwrap()
                .to_string()
        };
        assert_eq!(s1, s2);
        unsafe { etassia_free(b1) };
        unsafe { etassia_free(b2) };
    }
}
