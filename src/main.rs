use axum::{
    extract::DefaultBodyLimit,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum::extract::Multipart;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let upload_dir = Path::new("/tmp/uploads");
    if !upload_dir.exists() {
        fs::create_dir_all(upload_dir).unwrap();
    }

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/upload", post(upload_handler))
        .route("/download/:filename", get(download_ipa))
        .route("/manifest/:filename", get(manifest_handler))
        .layer(DefaultBodyLimit::max(500 * 1024 * 1024));

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index_handler() -> Html<&'static str> {
    Html(r#"
        <!DOCTYPE html>
        <html lang="ja">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>IPA Installer</title>
            <style>
                body { font-family: sans-serif; text-align: center; padding: 50px; background: #f4f4f9; }
                .card { background: white; padding: 30px; border-radius: 12px; box-shadow: 0 4px 10px rgba(0,0,0,0.1); display: inline-block; }
                input[type="file"] { margin: 20px 0; }
                button { background: #007aff; color: white; border: none; padding: 10px 20px; border-radius: 6px; font-size: 16px; cursor: pointer; }
                button:hover { background: #005bb5; }
            </style>
        </head>
        <body>
            <div class="card">
                <h2>iPad IPA インストーラー</h2>
                <form action="/upload" method="POST" enctype="multipart/form-data">
                    <input type="file" name="ipa" accept=".ipa" required><br>
                    <button type="submit">アップロードしてインストールリンク生成</button>
                </form>
            </div>
        </body>
        </html>
    "#)
}

async fn upload_handler(mut multipart: Multipart) -> Response {
    let upload_dir = Path::new("/tmp/uploads");
    let mut saved_filename = String::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("").to_string();
        if name == "ipa" {
            let original_name = field.file_name().unwrap_or("app.ipa").to_string();
            let ext = Path::new(&original_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("ipa");
            
            let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
            let path = upload_dir.join(&unique_name);

            let data = field.bytes().await.unwrap();
            fs::write(&path, data).unwrap();
            saved_filename = unique_name;
            break;
        }
    }

    if saved_filename.is_empty() {
        return (StatusCode::BAD_REQUEST, "No file uploaded").into_response();
    }

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="ja">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>インストール準備完了</title>
            <style>
                body {{ font-family: sans-serif; text-align: center; padding: 50px; background: #f4f4f9; }}
                .card {{ background: white; padding: 30px; border-radius: 12px; box-shadow: 0 4px 10px rgba(0,0,0,0.1); display: inline-block; }}
                .btn {{ background: #34c759; color: white; text-decoration: none; padding: 12px 24px; border-radius: 6px; font-size: 18px; display: inline-block; margin-top: 20px; }}
            </style>
        </head>
        <body>
            <div class="card">
                <h2>準備完了！</h2>
                <p>下のボタンをタップするとインストールが始まります。</p>
                <a class="btn" href="itms-services://?action=download-manifest&url=https://{{HOST}}/manifest/{}">インストールする</a>
            </div>
        </body>
        </html>
        "#,
        saved_filename
    )).into_response()
}

async fn manifest_handler(
    axum::extract::Path(filename): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8080");

    let ipa_url = format!("https://{}/download/{}", host, filename);

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>items</key>
    <array>
        <dict>
            <key>assets</key>
            <array>
                <dict>
                    <key>kind</key>
                    <string>software-package</string>
                    <key>url</key>
                    <string>{}</string>
                </dict>
            </array>
            <key>metadata</key>
            <dict>
                <key>bundle-identifier</key>
                <string>com.example.customipa</string>
                <key>bundle-version</key>
                <string>1.0.0</string>
                <key>kind</key>
                <string>software</string>
                <key>title</key>
                <string>Custom IPA App</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>
"__,
        ipa_url
    );

    Response::builder()
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(plist_content)
        .unwrap()
}

async fn download_ipa(
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Response {
    let filepath = format!("/tmp/uploads/{}", filename);
    if let Ok(data) = fs::read(filepath) {
        Response::builder()
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(data.into())
            .unwrap()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
