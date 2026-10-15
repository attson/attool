use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const MAX_REQUEST_HEADER_BYTES: usize = 16 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ByteRange {
    pub start: u64,
    pub end: u64,
}

pub(crate) fn parse_byte_range(value: &str, file_size: u64) -> Result<ByteRange, ()> {
    if file_size == 0 {
        return Err(());
    }
    let range = value.trim().strip_prefix("bytes=").ok_or(())?;
    if range.is_empty() || range.contains(',') {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;
    match (start.trim(), end.trim()) {
        ("", "") => Err(()),
        ("", suffix) => {
            let suffix = suffix.parse::<u64>().map_err(|_| ())?;
            if suffix == 0 {
                return Err(());
            }
            Ok(ByteRange {
                start: file_size.saturating_sub(suffix),
                end: file_size - 1,
            })
        }
        (start, "") => {
            let start = start.parse::<u64>().map_err(|_| ())?;
            if start >= file_size {
                return Err(());
            }
            Ok(ByteRange {
                start,
                end: file_size - 1,
            })
        }
        (start, end) => {
            let start = start.parse::<u64>().map_err(|_| ())?;
            let end = end.parse::<u64>().map_err(|_| ())?.min(file_size - 1);
            if start >= file_size || end < start {
                return Err(());
            }
            Ok(ByteRange { start, end })
        }
    }
}

#[derive(Clone)]
struct PreviewFile {
    path: PathBuf,
    content_type: &'static str,
}

struct RunningServer {
    port: u16,
    files: Arc<Mutex<HashMap<String, PreviewFile>>>,
}

#[derive(Default)]
pub(crate) struct RecordingPreviewServer {
    running: Mutex<Option<RunningServer>>,
}

impl RecordingPreviewServer {
    pub(crate) fn register(&self, path: PathBuf) -> Result<String, String> {
        if !path.is_file() {
            return Err("录屏缓存不存在".to_string());
        }
        let mut running = self
            .running
            .lock()
            .map_err(|_| "录屏预览服务状态不可用".to_string())?;
        if running.is_none() {
            *running = Some(start_server()?);
        }
        let running = running.as_ref().expect("preview server initialized");
        let token = Uuid::new_v4().simple().to_string();
        let content_type = match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("mp4") => "video/mp4",
            _ => "video/webm",
        };
        running
            .files
            .lock()
            .map_err(|_| "录屏预览文件状态不可用".to_string())?
            .insert(token.clone(), PreviewFile { path, content_type });
        Ok(format!(
            "http://127.0.0.1:{}/recording/{token}",
            running.port
        ))
    }

    pub(crate) fn unregister(&self, path: &Path) {
        let files = {
            let Ok(running) = self.running.lock() else {
                return;
            };
            let Some(running) = running.as_ref() else {
                return;
            };
            running.files.clone()
        };
        if let Ok(mut entries) = files.lock() {
            entries.retain(|_, entry| entry.path != path);
        };
    }
}

fn start_server() -> Result<RunningServer, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("启动录屏预览服务失败：{error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("读取录屏预览地址失败：{error}"))?
        .port();
    let files = Arc::new(Mutex::new(HashMap::new()));
    let server_files = files.clone();
    thread::Builder::new()
        .name("recording-preview".to_string())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    continue;
                };
                let files = server_files.clone();
                let _ = thread::Builder::new()
                    .name("recording-preview-request".to_string())
                    .spawn(move || {
                        let _ = serve_request(stream, &files);
                    });
            }
        })
        .map_err(|error| format!("启动录屏预览线程失败：{error}"))?;
    Ok(RunningServer { port, files })
}

fn serve_request(
    mut stream: TcpStream,
    files: &Arc<Mutex<HashMap<String, PreviewFile>>>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut header_bytes = request_line.len();
    let mut range_header = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        header_bytes += read;
        if header_bytes > MAX_REQUEST_HEADER_BYTES {
            return write_status(&mut stream, 431, "Request Header Fields Too Large", &[]);
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("range") {
                range_header = Some(value.trim().to_string());
            }
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let request_path = parts.next().unwrap_or_default();
    if !matches!(method, "GET" | "HEAD") {
        return write_status(&mut stream, 405, "Method Not Allowed", &[]);
    }
    let token = request_path
        .split('?')
        .next()
        .and_then(|path| path.strip_prefix("/recording/"))
        .filter(|value| !value.is_empty());
    let Some(token) = token else {
        return write_status(&mut stream, 404, "Not Found", &[]);
    };
    let file = files
        .lock()
        .ok()
        .and_then(|files| files.get(token).cloned());
    let Some(file) = file else {
        return write_status(&mut stream, 404, "Not Found", &[]);
    };

    let mut source = match File::open(&file.path) {
        Ok(source) => source,
        Err(_) => return write_status(&mut stream, 404, "Not Found", &[]),
    };
    let file_size = source.metadata()?.len();
    let range = match range_header {
        Some(value) => match parse_byte_range(&value, file_size) {
            Ok(range) => Some(range),
            Err(()) => {
                return write_status(
                    &mut stream,
                    416,
                    "Range Not Satisfiable",
                    &[("Content-Range", format!("bytes */{file_size}"))],
                )
            }
        },
        None => None,
    };
    let (status, reason, start, end) = match range {
        Some(range) => (206, "Partial Content", range.start, range.end),
        None => (200, "OK", 0, file_size.saturating_sub(1)),
    };
    let content_length = if file_size == 0 { 0 } else { end - start + 1 };
    let mut headers = vec![
        ("Content-Type", file.content_type.to_string()),
        ("Content-Length", content_length.to_string()),
        ("Accept-Ranges", "bytes".to_string()),
        ("Access-Control-Allow-Origin", "*".to_string()),
        ("Cross-Origin-Resource-Policy", "cross-origin".to_string()),
        ("Cache-Control", "no-store".to_string()),
    ];
    if status == 206 {
        headers.push(("Content-Range", format!("bytes {start}-{end}/{file_size}")));
    }
    write_headers(&mut stream, status, reason, &headers)?;
    if method == "HEAD" || content_length == 0 {
        return Ok(());
    }
    source.seek(SeekFrom::Start(start))?;
    io::copy(&mut source.take(content_length), &mut stream)?;
    Ok(())
}

fn write_status(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    extra_headers: &[(&str, String)],
) -> io::Result<()> {
    let mut headers = vec![
        ("Content-Length", "0".to_string()),
        ("Access-Control-Allow-Origin", "*".to_string()),
        ("Cache-Control", "no-store".to_string()),
    ];
    headers.extend_from_slice(extra_headers);
    write_headers(stream, status, reason, &headers)
}

fn write_headers(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    headers: &[(&str, String)],
) -> io::Result<()> {
    write!(stream, "HTTP/1.1 {status} {reason}\r\n")?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "Connection: close\r\n\r\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounded_and_open_byte_ranges() {
        assert_eq!(
            parse_byte_range("bytes=10-19", 100),
            Ok(ByteRange { start: 10, end: 19 })
        );
        assert_eq!(
            parse_byte_range("bytes=90-", 100),
            Ok(ByteRange { start: 90, end: 99 })
        );
    }

    #[test]
    fn parses_suffix_byte_ranges() {
        assert_eq!(
            parse_byte_range("bytes=-12", 100),
            Ok(ByteRange { start: 88, end: 99 })
        );
        assert_eq!(
            parse_byte_range("bytes=-200", 100),
            Ok(ByteRange { start: 0, end: 99 })
        );
    }

    #[test]
    fn rejects_invalid_or_multiple_ranges() {
        for value in [
            "items=0-1",
            "bytes=",
            "bytes=20-10",
            "bytes=100-",
            "bytes=0-1,5-6",
        ] {
            assert_eq!(parse_byte_range(value, 100), Err(()));
        }
        assert_eq!(parse_byte_range("bytes=0-1", 0), Err(()));
    }

    #[test]
    fn streams_registered_file_ranges() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preview.webm");
        std::fs::write(&path, b"0123456789").unwrap();
        let server = RecordingPreviewServer::default();
        let url = server.register(path).unwrap();
        let address = url
            .strip_prefix("http://")
            .unwrap()
            .split('/')
            .next()
            .unwrap();
        let route = url.strip_prefix(&format!("http://{address}")).unwrap();
        let mut stream = TcpStream::connect(address).unwrap();
        write!(
            stream,
            "GET {route} HTTP/1.1\r\nHost: {address}\r\nRange: bytes=2-5\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 206 Partial Content\r\n"));
        assert!(response.contains("Content-Range: bytes 2-5/10\r\n"));
        assert!(response.ends_with("\r\n\r\n2345"));
    }
}
