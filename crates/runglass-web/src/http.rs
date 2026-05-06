use tiny_http::{Header, Response, StatusCode};

pub(crate) fn html_response(body: &str, content_type: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", content_type).expect("valid header");
    Response::from_string(body.to_owned()).with_header(header)
}

pub(crate) fn binary_response(
    bytes: &'static [u8],
    content_type: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", content_type).expect("valid header");
    Response::from_data(bytes.to_vec()).with_header(header)
}

pub(crate) fn json_status_response(
    status: StatusCode,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    html_response(body, "application/json").with_status_code(status)
}
