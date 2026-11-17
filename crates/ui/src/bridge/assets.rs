use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use include_dir::{Dir, include_dir};

static ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets");

pub const CSP: &str = "default-src 'self'; connect-src 'self' ws://127.0.0.1:*; \
img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'; \
object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

pub fn index() -> Response {
    serve("index.html")
}

pub fn asset(path: &str) -> Response {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return index();
    }
    if ASSETS.get_file(path).is_none() && !path.contains('.') {
        return index();
    }
    serve(path)
}

fn serve(path: &str) -> Response {
    let Some(file) = ASSETS.get_file(path) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut headers = security_headers();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    (headers, Body::from(file.contents())).into_response()
}

pub fn security_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}
